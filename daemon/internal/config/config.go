package config

import (
	"os"
	"os/user"
	"path/filepath"

	"github.com/sblinch/kdl-go"
)

type Config struct {
	Port         int    `kdl:"port"`
	CertFile     string `kdl:"cert_file,optional"`
	KeyFile      string `kdl:"key_file,optional"`
	ProjectsPath string `kdl:"projects_path,optional"`
}

func Load(path string) (*Config, error) {
	f, err := os.Open(path)
	if err != nil {
		return nil, err
	}
	defer f.Close()

	var cfg Config
	if err := kdl.NewDecoder(f).Decode(&cfg); err != nil {
		return nil, err
	}

	return &cfg, nil
}

func Default() *Config {
	usr, err := user.Current()
	projectsPath := ""
	if err == nil {
		projectsPath = filepath.Join(usr.HomeDir, "code")
	}

	return &Config{
		Port:         8083,
		CertFile:     "",
		KeyFile:      "",
		ProjectsPath: projectsPath,
	}
}