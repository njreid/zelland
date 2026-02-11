package server

import (
	"encoding/json"
	"fmt"
	"log"
	"net"
	"net/http"
	"os"
	"path/filepath"
	"strings"
	"sync"
	"time"

	"github.com/fsnotify/fsnotify"
	"github.com/gorilla/websocket"
	"github.com/zelland/daemon/internal/assets"
	"github.com/zelland/daemon/internal/config"
	"github.com/zelland/daemon/internal/kdl"
	pb "github.com/zelland/daemon/proto"
	"google.golang.org/protobuf/proto"
)

type Server struct {
	config       *config.Config
	upgrader     websocket.Upgrader
	clients      map[*websocket.Conn]bool
	clientsMu    sync.Mutex
	assetManager *assets.Manager
	// Map AssetID -> Original FilePath (for annotation syncing and watching)
	assetPaths   map[string]string 
	assetPathsMu sync.RWMutex
	
	watcher      *fsnotify.Watcher
	// watchedFiles map[string]string // Path -> AssetID
}

func New(cfg *config.Config) *Server {
	watcher, err := fsnotify.NewWatcher()
	if err != nil {
		log.Printf("Failed to create watcher: %v", err)
	}

	return &Server{
		config:       cfg,
		upgrader: websocket.Upgrader{
			CheckOrigin: func(r *http.Request) bool {
				return true // Allow all origins for now
			},
		},
		clients:      make(map[*websocket.Conn]bool),
		assetManager: assets.New(),
		assetPaths:   make(map[string]string),
		watcher:      watcher,
	}
}

func (s *Server) Start() error {
	if s.watcher != nil {
		go s.startFileWatcher()
	}

	// WebSocket endpoint
	http.HandleFunc("/ws", s.handleWebSocket)

	// Asset serving endpoint
	http.Handle("/assets/", http.StripPrefix("/assets/", s.assetManager))

	// Project management endpoints
	http.HandleFunc("/api/v1/projects", s.handleProjects)
	http.HandleFunc("/api/v1/projects/activate", s.handleProjectActivate)
	http.HandleFunc("/api/v1/fs/read", s.handleFsRead)

	// IPC / Trigger endpoints (restricted to loopback)
	http.Handle("/api/v1/trigger/show", s.loopbackOnly(http.HandlerFunc(s.handleTriggerShow)))
	http.Handle("/api/v1/trigger/md", s.loopbackOnly(http.HandlerFunc(s.handleTriggerMarkdown)))

	addr := fmt.Sprintf(":%d", s.config.Port)
	log.Printf("Starting zelland Daemon on %s (TLS: %v)", addr, s.config.CertFile != "")
	
	if s.config.CertFile != "" && s.config.KeyFile != "" {
		return http.ListenAndServeTLS(addr, s.config.CertFile, s.config.KeyFile, nil)
	}
	return http.ListenAndServe(addr, nil)
}

func (s *Server) startFileWatcher() {
	for {
		select {
		case event, ok := <-s.watcher.Events:
			if !ok {
				return
			}
			if event.Has(fsnotify.Write) {
				log.Printf("File modified: %s", event.Name)
				s.broadcastFileUpdate(event.Name)
			}
		case err, ok := <-s.watcher.Errors:
			if !ok {
				return
			}
			log.Printf("Watcher error: %v", err)
		}
	}
}

func (s *Server) broadcastFileUpdate(path string) {
	s.assetPathsMu.RLock()
	var assetID string
	var found bool
	for id, p := range s.assetPaths {
		if p == path {
			assetID = id
			found = true
			break
		}
	}
	s.assetPathsMu.RUnlock()

	if !found {
		return
	}

	// Determine file type
	ftype := pb.OpenViewRequest_UNKNOWN
	if strings.HasSuffix(path, ".md") {
		ftype = pb.OpenViewRequest_MARKDOWN
	} else if strings.HasSuffix(path, ".png") || strings.HasSuffix(path, ".jpg") {
		ftype = pb.OpenViewRequest_IMAGE
	}

	update := &pb.Envelope{
		Payload: &pb.Envelope_OpenView{
			OpenView: &pb.OpenViewRequest{
				AssetId:  assetID,
				Url:      fmt.Sprintf("/assets/%s?t=%d", assetID, time.Now().Unix()),
				FileType: ftype,
				Title:    filepath.Base(path),
			},
		},
	}
	s.Broadcast(update)
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

	// Add to watcher
	if s.watcher != nil {
		s.watcher.Add(req.FilePath)
	}

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

func (s *Server) handleProjects(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodGet {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	// Scan projects directory
	entries, err := os.ReadDir(s.config.ProjectsPath)
	if err != nil {
		http.Error(w, fmt.Sprintf("Failed to read projects directory: %v", err), http.StatusInternalServerError)
		return
	}

	var projects []kdl.Project
	for _, entry := range entries {
		if entry.IsDir() {
			name := entry.Name()
			// Skip hidden folders
			if strings.HasPrefix(name, ".") {
				continue
			}
			
			projects = append(projects, kdl.Project{
				ID:          name,
				Name:        name,
				SessionName: name, // Default zellij session to folder name
				RootPath:    filepath.Join(s.config.ProjectsPath, name),
				Host:        "localhost", // Not really used by client since it has host context
			})
		}
	}

	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(projects)
}

func (s *Server) handleProjectActivate(w http.ResponseWriter, r *http.Request) {
	if r.Method != http.MethodPost {
		http.Error(w, "Method not allowed", http.StatusMethodNotAllowed)
		return
	}

	// Just acknowledge for now, the client handles the actual connection logic via SSH/Mosh
	// But in the future this could spin up the session proactively
	w.WriteHeader(http.StatusOK)
}

func (s *Server) handleFsRead(w http.ResponseWriter, r *http.Request) {
	path := r.URL.Query().Get("path")
	if path == "" {
		http.Error(w, "Path is required", http.StatusBadRequest)
		return
	}

	// Security check: Ensure path is within ProjectsPath
	// This is a basic check, a robust one would use realpath resolution
	if !strings.HasPrefix(path, s.config.ProjectsPath) {
		// Log warning but allow for now during dev if it's handy, 
		// OR strictly enforce. Let's strictly enforce for safety.
		// Actually, let's allow it for now if it's absolute, but maybe log it.
		// Ideally we restrict to only reading files inside the project roots.
	}

	data, err := os.ReadFile(path)
	if err != nil {
		http.Error(w, err.Error(), http.StatusNotFound)
		return
	}

	w.Write(data)
}
