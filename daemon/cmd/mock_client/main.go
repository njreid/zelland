package main

import (
	"log"
	"net/url"
	"os"
	"os/signal"
	"time"

	"github.com/gorilla/websocket"
	pb "github.com/zelland/daemon/proto"
	"google.golang.org/protobuf/proto"
)

func main() {
	interrupt := make(chan os.Signal, 1)
	signal.Notify(interrupt, os.Interrupt)

	u := url.URL{Scheme: "ws", Host: "localhost:8083", Path: "/ws"}
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

			switch payload := env.Payload.(type) {
			case *pb.Envelope_Ping:
				log.Printf("Received PING: %d", payload.Ping.Timestamp)
			case *pb.Envelope_OpenView:
				log.Printf(">>> OPEN VIEW REQUEST <<<")
				log.Printf("ID:    %s", payload.OpenView.AssetId)
				log.Printf("URL:   %s", payload.OpenView.Url)
				log.Printf("Title: %s", payload.OpenView.Title)
				log.Printf("Type:  %s", payload.OpenView.FileType)
				
				// Simulate creating an annotation if it's a markdown file
				if payload.OpenView.FileType == pb.OpenViewRequest_MARKDOWN {
					go func() {
						time.Sleep(2 * time.Second)
						log.Println("Simulating annotation creation...")
						sendAnnotation(c, payload.OpenView.AssetId)
					}()
				}
				
			default:
				log.Printf("Received unknown message: %T", payload)
			}
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
	c.WriteMessage(websocket.BinaryMessage, data)
}
