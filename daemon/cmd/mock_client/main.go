package main

import (
	"flag"
	"fmt"
	"io"
	"log"
	"net/http"
	"net/url"
	"os"
	"os/signal"
	"time"

	"github.com/gorilla/websocket"
	pb "github.com/zelland/daemon/proto"
	"google.golang.org/protobuf/proto"
)

var (
	host = flag.String("host", "localhost", "Daemon host")
	port = flag.Int("port", 8083, "Daemon port")
)

func main() {
	flag.Parse()

	interrupt := make(chan os.Signal, 1)
	signal.Notify(interrupt, os.Interrupt)

	addr := fmt.Sprintf("%s:%d", *host, *port)
	u := url.URL{Scheme: "ws", Host: addr, Path: "/ws"}
	log.Printf("Connecting to %s", u.String())

	c, _, err := websocket.DefaultDialer.Dial(u.String(), nil)
	if err != nil {
		log.Fatal("dial:", err)
	}
	defer c.Close()

	done := make(chan struct{})

	go func() {
		defer close(done)
		for {
			_, message, err := c.ReadMessage()
			if err != nil {
				log.Println("read:", err)
				return
			}
			
			var env pb.Envelope
			if err := proto.Unmarshal(message, &env); err != nil {
				log.Println("unmarshal:", err)
				continue
			}

			handleMessage(c, &env, addr)
		}
	}()

	for {
		select {
		case <-interrupt:
			log.Println("interrupt")
			err := c.WriteMessage(websocket.CloseMessage, websocket.FormatCloseMessage(websocket.CloseNormalClosure, ""))
			if err != nil {
				log.Println("write close:", err)
				return
			}
			select {
			case <-done:
			case <-time.After(time.Second):
			}
			return
		case <-done:
			return
		}
	}
}

func handleMessage(c *websocket.Conn, env *pb.Envelope, hostAddr string) {
	switch payload := env.Payload.(type) {
	case *pb.Envelope_Ping:
		log.Printf("[PING] Timestamp: %d", payload.Ping.Timestamp)
		// Ideally send a Pong back if defined in proto, currently just logging

	case *pb.Envelope_OpenView:
		log.Printf(">>> OPEN VIEW REQUEST <<<")
		log.Printf("  ID:    %s", payload.OpenView.AssetId)
		log.Printf("  Title: %s", payload.OpenView.Title)
		log.Printf("  Type:  %s", payload.OpenView.FileType)
		log.Printf("  URL:   %s", payload.OpenView.Url)

		// Verify asset accessibility
		go verifyAsset(hostAddr, payload.OpenView.Url)

		// Simulate user interaction for Markdown
		if payload.OpenView.FileType == pb.OpenViewRequest_MARKDOWN {
			go func() {
				log.Println("  [Sim] User reading...")
				time.Sleep(2 * time.Second)
				log.Println("  [Sim] User creating annotation...")
				sendAnnotation(c, payload.OpenView.AssetId)
			}()
		}

	case *pb.Envelope_Annotation:
		log.Printf(">>> ANNOTATION RECEIVED <<<")
		log.Printf("  File: %s", payload.Annotation.FilePath)
		log.Printf("  Type: %s", payload.Annotation.Type)
		if payload.Annotation.Data != nil {
			log.Printf("  Body: %s", payload.Annotation.Data.Body)
		}

	default:
		log.Printf("Received unknown message: %T", payload)
	}
}

func verifyAsset(hostAddr, path string) {
	fullURL := fmt.Sprintf("http://%s%s", hostAddr, path)
	log.Printf("  [Verify] Fetching %s...", fullURL)
	
	resp, err := http.Get(fullURL)
	if err != nil {
		log.Printf("  [Verify] FAILED: %v", err)
		return
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		log.Printf("  [Verify] FAILED: Status %d", resp.StatusCode)
		return
	}

	body, _ := io.ReadAll(resp.Body)
	log.Printf("  [Verify] SUCCESS: Status %d, Size: %d bytes", resp.StatusCode, len(body))
}

func sendAnnotation(c *websocket.Conn, assetID string) {
	ann := &pb.Envelope{
		Payload: &pb.Envelope_Annotation{
			Annotation: &pb.AnnotationAction{
				Type:     pb.AnnotationAction_CREATE,
				FilePath: assetID, // Using AssetID as reference
				Data: &pb.AnnotationData{
					Id:          "simulated-ann-" + time.Now().Format("150405"),
					TargetText:  "This is interesting",
					ContextHash: "sha256:dummy",
					Body:        "Simulated annotation from Mock Client",
					Timestamp:   time.Now().Unix(),
				},
			},
		},
	}
	
	data, _ := proto.Marshal(ann)
	if err := c.WriteMessage(websocket.BinaryMessage, data); err != nil {
		log.Printf("Failed to send annotation: %v", err)
	} else {
		log.Printf("  [Sent] Annotation created.")
	}
}