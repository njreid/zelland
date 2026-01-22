package kdl

import (
	"os"
	"path/filepath"
	"testing"
)

func TestKDLSerialization(t *testing.T) {
	tempDir, err := os.MkdirTemp("", "kdl-test")
	if err != nil {
		t.Fatalf("Failed to create temp dir: %v", err)
	}
	defer os.RemoveAll(tempDir)

	kdlPath := filepath.Join(tempDir, "test.kdl")

	expected := []Annotation{
		{
			ID:          "ann-1",
			User:        "alice",
			Timestamp:   123456789,
			ContextHash: "sha256:abc",
			TargetText:  "Hello",
			Body:        "World",
		},
		{
			ID:          "ann-2",
			User:        "bob",
			Timestamp:   987654321,
			ContextHash: "sha256:def",
			TargetText:  "Foo",
			Body:        "Bar",
		},
	}

	// Test Save
	if err := Save(kdlPath, expected); err != nil {
		t.Fatalf("Save failed: %v", err)
	}

	content, _ := os.ReadFile(kdlPath)
	t.Logf("KDL Content:\n%s", string(content))

	// Test Load
	actual, err := Load(kdlPath)
	if err != nil {
		t.Fatalf("Load failed: %v", err)
	}

	if len(actual) != len(expected) {
		t.Fatalf("Expected %d annotations, got %d", len(expected), len(actual))
	}

	for i := range expected {
		if actual[i].ID != expected[i].ID ||
			actual[i].Body != expected[i].Body ||
			actual[i].TargetText != expected[i].TargetText {
			t.Errorf("Annotation mismatch at index %d: expected %+v, got %+v", i, expected[i], actual[i])
		}
	}

	// Test Append (Update)
	updatedAnn := Annotation{
		ID:         "ann-1",
		TargetText: "Hello Updated",
		Body:       "World Updated",
	}
	if err := Append(kdlPath, updatedAnn); err != nil {
		t.Fatalf("Append (update) failed: %v", err)
	}

	anns, _ := Load(kdlPath)
	if len(anns) != 2 {
		t.Errorf("Expected 2 annotations after update, got %d", len(anns))
	}
	if anns[0].Body != "World Updated" {
		t.Errorf("Expected updated body 'World Updated', got '%s'", anns[0].Body)
	}

	// Test Append (New)
	newAnn := Annotation{
		ID:         "ann-3",
		TargetText: "New",
		Body:       "Note",
	}
	if err := Append(kdlPath, newAnn); err != nil {
		t.Fatalf("Append (new) failed: %v", err)
	}

	anns, _ = Load(kdlPath)
	if len(anns) != 3 {
		t.Errorf("Expected 3 annotations after append, got %d", len(anns))
	}
}
