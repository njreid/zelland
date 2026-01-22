package assets

import (
	"crypto/rand"
	"encoding/hex"
	"net/http"
	"os"
	"path/filepath"
	"sync"
	"time"
)

type assetEntry struct {
	filePath  string
	expiresAt time.Time
}

type Manager struct {
	assets map[string]assetEntry
	mu     sync.RWMutex
}

func New() *Manager {
	m := &Manager{
		assets: make(map[string]assetEntry),
	}
	go m.cleanupRoutine()
	return m
}

// Register adds a file to the asset manager and returns its ID.
// Assets expire after 30 minutes by default.
func (m *Manager) Register(filePath string) (string, error) {
	absPath, err := filepath.Abs(filePath)
	if err != nil {
		return "", err
	}

	if _, err := os.Stat(absPath); err != nil {
		return "", err
	}

	id := generateID()

	m.mu.Lock()
	m.assets[id] = assetEntry{
		filePath:  absPath,
		expiresAt: time.Now().Add(30 * time.Minute),
	}
	m.mu.Unlock()

	return id, nil
}

// ServeHTTP handles requests for /assets/{id}
func (m *Manager) ServeHTTP(w http.ResponseWriter, r *http.Request) {
	id := filepath.Base(r.URL.Path)

	m.mu.RLock()
	entry, ok := m.assets[id]
	m.mu.RUnlock()

	if !ok || time.Now().After(entry.expiresAt) {
		http.NotFound(w, r)
		return
	}

	http.ServeFile(w, r, entry.filePath)
}

func generateID() string {
	b := make([]byte, 8) // Increased to 8 bytes for more entropy
	rand.Read(b)
	return hex.EncodeToString(b)
}

func (m *Manager) cleanupRoutine() {
	ticker := time.NewTicker(5 * time.Minute)
	for range ticker.C {
		m.mu.Lock()
		now := time.Now()
		for id, entry := range m.assets {
			if now.After(entry.expiresAt) {
				delete(m.assets, id)
			}
		}
		m.mu.Unlock()
	}
}