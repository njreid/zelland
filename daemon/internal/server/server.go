package server

import (
	"encoding/json"
	"fmt"
	"log"
	"net"
	"net/http"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/gorilla/websocket"
	"github.com/zelland/daemon/internal/assets"
	"github.com/zelland/daemon/internal/kdl"
	pb "github.com/zelland/daemon/proto"
	"google.golang.org/protobuf/proto"
)

type Server struct {
	port         int
	certFile     string
	keyFile      string
	upgrader     websocket.Upgrader
	clients      map[*websocket.Conn]bool
	clientsMu    sync.Mutex
	assetManager *assets.Manager
	// Map AssetID -> Original FilePath (for annotation syncing)
	assetPaths   map[string]string 
	assetPathsMu sync.RWMutex
}

func New(port int, certFile, keyFile string) *Server {
	return &Server{
		port:         port,
		certFile:     certFile,
		keyFile:      keyFile,
		upgrader: websocket.Upgrader{
			CheckOrigin: func(r *http.Request) bool {
				return true // Allow all origins for now
			},
		},
		clients:      make(map[*websocket.Conn]bool),
		assetManager: assets.New(),
		assetPaths:   make(map[string]string),
	}
}

func (s *Server) Start() error {
	// WebSocket endpoint
	http.HandleFunc("/ws", s.handleWebSocket)

	// Asset serving endpoint
	http.Handle("/assets/", http.StripPrefix("/assets/", s.assetManager))

	// IPC / Trigger endpoints (restricted to loopback)
	http.Handle("/api/v1/trigger/show", s.loopbackOnly(http.HandlerFunc(s.handleTriggerShow)))
	http.Handle("/api/v1/trigger/md", s.loopbackOnly(http.HandlerFunc(s.handleTriggerMarkdown)))

	addr := fmt.Sprintf(":%d", s.port)
	log.Printf("Starting Zelland Daemon on %s (TLS: %v)", addr, s.certFile != "")
	
	if s.certFile != "" && s.keyFile != "" {
		return http.ListenAndServeTLS(addr, s.certFile, s.keyFile, nil)
	}
	return http.ListenAndServe(addr, nil)
}

func (s *Server) loopbackOnly(next http.Handler) http.Handler {
	return http.HandlerFunc(func(w http.ResponseWriter, r *http.Request) {
		host, _, err := net.SplitHostPort(r.RemoteAddr)
		if err != nil {
			// If split fails, we can't trust the address
			http.Error(w, "Forbidden", http.StatusForbidden)
			return
		}

		if host != "127.0.0.1" && host != "::1" && host != "localhost" {
			log.Printf("Blocked non-loopback access to %s from %s", r.URL.Path, r.RemoteAddr)
			http.Error(w, "Forbidden", http.StatusForbidden)
			return
		}
		next.ServeHTTP(w, r)
	})
}

// IPC Request Body
type ShowRequest struct {
	FilePath string `json:"file_path"`
	Title    string `json:"title"`
}

func (s *Server) handleTriggerShow(w http.ResponseWriter, r *http.Request) {
	s.genericTrigger(w, r, pb.OpenViewRequest_IMAGE)
}

func (s *Server) handleTriggerMarkdown(w http.ResponseWriter, r *http.Request) {
	s.genericTrigger(w, r, pb.OpenViewRequest_MARKDOWN)
}

func (s *Server) genericTrigger(w http.ResponseWriter, r *http.Request, ftype pb.OpenViewRequest_FileType) {
	if r.Method != http.MethodPost {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	var req ShowRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, err.Error(), http.StatusBadRequest)
		return
	}

	// Register file
	assetID, err := s.assetManager.Register(req.FilePath)
	if err != nil {
		log.Printf("Failed to register asset: %v", err)
		http.Error(w, fmt.Sprintf("Failed to access file: %v", err), http.StatusBadRequest)
		return
	}
	
	s.assetPathsMu.Lock()
	s.assetPaths[assetID] = req.FilePath
	s.assetPathsMu.Unlock()

	// Construct URL
	assetURL := fmt.Sprintf("/assets/%s", assetID)

	// Broadcast to clients
	viewReq := &pb.Envelope{
		Payload: &pb.Envelope_OpenView{
			OpenView: &pb.OpenViewRequest{
				AssetId:  assetID,
				Url:      assetURL,
				FileType: ftype,
				Title:    req.Title,
			},
		},
	}

	s.Broadcast(viewReq)

	w.WriteHeader(http.StatusOK)
	fmt.Fprintf(w, "Showing %s (ID: %s)", req.FilePath, assetID)
}

func (s *Server) handleWebSocket(w http.ResponseWriter, r *http.Request) {
	conn, err := s.upgrader.Upgrade(w, r, nil)
	if err != nil {
		log.Printf("Upgrade error: %v", err)
		return
	}
	defer conn.Close()

	s.registerClient(conn)
	defer s.unregisterClient(conn)

	log.Printf("Client connected: %s", conn.RemoteAddr())

	// Send a welcome ping
	s.sendPing(conn)

	for {
		_, message, err := conn.ReadMessage()
		if err != nil {
			log.Printf("Read error: %v", err)
			break
		}
		
		var env pb.Envelope
		if err := proto.Unmarshal(message, &env); err != nil {
			log.Printf("Unmarshal error: %v", err)
			continue
		}

		s.handleMessage(conn, &env)
	}
}

func (s *Server) registerClient(conn *websocket.Conn) {
	s.clientsMu.Lock()
	defer s.clientsMu.Unlock()
	s.clients[conn] = true
}

func (s *Server) unregisterClient(conn *websocket.Conn) {
	s.clientsMu.Lock()
	defer s.clientsMu.Unlock()
	delete(s.clients, conn)
	log.Printf("Client disconnected: %s", conn.RemoteAddr())
}

func (s *Server) sendPing(conn *websocket.Conn) {
	ping := &pb.Envelope{
		Payload: &pb.Envelope_Ping{
			Ping: &pb.KeepAlive{
				Timestamp: time.Now().Unix(),
			},
		},
	}
	s.SendMessage(conn, ping)
}

func (s *Server) SendMessage(conn *websocket.Conn, env *pb.Envelope) {
	data, err := proto.Marshal(env)
	if err != nil {
		log.Printf("Marshal error: %v", err)
		return
	}

	if err := conn.WriteMessage(websocket.BinaryMessage, data); err != nil {
		log.Printf("Write error: %v", err)
	}
}

func (s *Server) handleMessage(conn *websocket.Conn, env *pb.Envelope) {
	switch payload := env.Payload.(type) {
	case *pb.Envelope_Annotation:
		s.handleAnnotation(payload.Annotation)
	default:
		log.Printf("Received message: %T", payload)
	}
}

func (s *Server) handleAnnotation(action *pb.AnnotationAction) {
	s.assetPathsMu.RLock()
	filePath, ok := s.assetPaths[action.FilePath] // client sends assetID as FilePath in proto
	if !ok {
		// Fallback: maybe the client sent the actual path?
		filePath = action.FilePath
	}
	s.assetPathsMu.RUnlock()

	// Determine KDL path
	// If filePath is /foo/bar.md, kdl is /foo/bar.kdl
	ext := filepath.Ext(filePath)
	kdlPath := strings.TrimSuffix(filePath, ext) + ".kdl"
	
	ann := kdl.Annotation{
		ID:          action.Data.Id,
		ContextHash: action.Data.ContextHash,
		TargetText:  action.Data.TargetText,
		Body:        action.Data.Body,
		Timestamp:   action.Data.Timestamp,
	}

	if err := kdl.Append(kdlPath, ann); err != nil {
		log.Printf("Failed to save annotation to %s: %v", kdlPath, err)
	} else {
		log.Printf("Saved annotation to %s", kdlPath)
	}
}

func (s *Server) Broadcast(env *pb.Envelope) {
	s.clientsMu.Lock()
	defer s.clientsMu.Unlock()

	for conn := range s.clients {
		go s.SendMessage(conn, env)
	}
}
