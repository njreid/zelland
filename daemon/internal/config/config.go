package config

import (
	"encoding/json"
	"os"
)

type Config struct {
	Port     int    `json:"port"`
	CertFile string `json:"cert_file"`
	KeyFile  string `json:"key_file"`
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
		Port:     8083,
		CertFile: "",
		KeyFile:  "",
	}
}
