package main

import (
	"bytes"
	"encoding/json"
	"fmt"
	"io"
	"net/http"
	"os"
	"path/filepath"
)

// IPC Request structure matching the server
type ShowRequest struct {
	FilePath string `json:"file_path"`
	Title    string `json:"title"`
}

func main() {
	if len(os.Args) < 2 {
		printUsage()
		os.Exit(1)
	}

	command := os.Args[1]
	switch command {
	case "show":
		handleShow(os.Args[2:])
	case "md":
		handleMarkdown(os.Args[2:])
	default:
		fmt.Printf("Unknown command: %s\n", command)
		printUsage()
		os.Exit(1)
	}
}

func printUsage() {
	fmt.Println("Usage: zelland <command> [args]")
	fmt.Println("Commands:")
	fmt.Println("  show <file>   Display a file on the connected device")
	fmt.Println("  md   <file>   Open a markdown session with annotations")
}

func handleShow(args []string) {
	trigger(args, "show")
}

func handleMarkdown(args []string) {
	trigger(args, "md")
}

func trigger(args []string, endpointType string) {
	if len(args) < 1 {
		fmt.Printf("Usage: zelland %s <filename>\n", endpointType)
		os.Exit(1)
	}

	filename := args[0]
	absPath, err := filepath.Abs(filename)
	if err != nil {
		fmt.Printf("Error resolving path: %v\n", err)
		os.Exit(1)
	}

	reqBody := ShowRequest{
		FilePath: absPath,
		Title:    filepath.Base(filename),
	}

	jsonData, err := json.Marshal(reqBody)
	if err != nil {
		fmt.Printf("Error marshaling request: %v\n", err)
		os.Exit(1)
	}

	url := fmt.Sprintf("http://localhost:8083/api/v1/trigger/%s", endpointType)
	resp, err := http.Post(url, "application/json", bytes.NewBuffer(jsonData))
	if err != nil {
		fmt.Printf("Error connecting to daemon: %v\nIs zellandd running?\n", err)
		os.Exit(1)
	}
	defer resp.Body.Close()

	if resp.StatusCode != http.StatusOK {
		body, _ := io.ReadAll(resp.Body)
		fmt.Printf("Error from daemon (Status %d): %s\n", resp.StatusCode, string(body))
		os.Exit(1)
	}

	fmt.Printf("Sent %s to device via %s.\n", filename, endpointType)
}
