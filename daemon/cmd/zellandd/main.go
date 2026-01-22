package main

import (
	"flag"
	"log"

	"github.com/zelland/daemon/internal/config"
	"github.com/zelland/daemon/internal/server"
)

func main() {
	configPath := flag.String("config", "", "Path to config file (JSON)")
	port := flag.Int("port", 0, "Port to listen on (overrides config)")
	flag.Parse()

	cfg := config.Default()
	if *configPath != "" {
		fileCfg, err := config.Load(*configPath)
		if err != nil {
			log.Fatalf("Failed to load config: %v", err)
		}
		cfg = fileCfg
	}

	if *port != 0 {
		cfg.Port = *port
	}

	srv := server.New(cfg.Port)
	if err := srv.Start(); err != nil {
		log.Fatalf("Server failed: %v", err)
	}
}