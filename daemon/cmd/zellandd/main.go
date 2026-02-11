package main

import (
	"flag"
	"log"
	"os"

	"github.com/zelland/daemon/internal/config"
	"github.com/zelland/daemon/internal/server"
)

func main() {
	configPath := flag.String("config", "daemon.kdl", "Path to config file (KDL)")
	port := flag.Int("port", 0, "Port to listen on (overrides config)")
	flag.Parse()

	cfg := config.Default()
	
	// Check if config file exists
	if _, err := os.Stat(*configPath); err == nil {
		fileCfg, err := config.Load(*configPath)
		if err != nil {
			log.Fatalf("Failed to load config: %v", err)
		}
		// Merge file config into default (simplified overwrite for now)
		if fileCfg.Port != 0 {
			cfg.Port = fileCfg.Port
		}
		if fileCfg.CertFile != "" {
			cfg.CertFile = fileCfg.CertFile
		}
		if fileCfg.KeyFile != "" {
			cfg.KeyFile = fileCfg.KeyFile
		}
		if fileCfg.ProjectsPath != "" {
			cfg.ProjectsPath = fileCfg.ProjectsPath
		}
	} else if *configPath != "daemon.kdl" {
		// Only fail if a non-default config was explicitly specified but missing
		log.Fatalf("Config file not found: %s", *configPath)
	}

	if *port != 0 {
		cfg.Port = *port
	}

	log.Printf("Using projects path: %s", cfg.ProjectsPath)

	srv := server.New(cfg)
	if err := srv.Start(); err != nil {
		log.Fatalf("Server failed: %v", err)
	}
}
