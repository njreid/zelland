package config

import (
	"encoding/json"
	"os"
)

type Config struct {
	Port int `json:"port"`
	// Add more fields as needed (e.g. FCM keys, storage paths)
}

func Load(path string) (*Config, error) {
	data, err := os.ReadFile(path)
	if err != nil {
		return nil, err
	}

	var cfg Config
	if err := json.Unmarshal(data, &cfg); err != nil {
		return nil, err
	}

	return &cfg, nil
}

func Default() *Config {
	return &Config{
		Port: 8083,
	}
}
