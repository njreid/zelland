package kdl

import (
	"os"

	"github.com/sblinch/kdl-go"
)

type Annotation struct {
	ID        string `kdl:"id,prop"`
	User      string `kdl:"user,prop,optional"`
	Timestamp int64  `kdl:"timestamp,prop,optional"`

	// Children nodes
	ContextHash string `kdl:"context_hash,child"`
	TargetText  string `kdl:"target_text,child"`
	Body        string `kdl:"body,child"`
}

type KDLFile struct {
	Annotations []Annotation `kdl:"annotation,multiple"`
}

func Load(path string) ([]Annotation, error) {
	f, err := os.Open(path)
	if os.IsNotExist(err) {
		return []Annotation{}, nil
	}
	if err != nil {
		return nil, err
	}
	defer f.Close()

	var doc KDLFile
	if err := kdl.NewDecoder(f).Decode(&doc); err != nil {
		return nil, err
	}

	return doc.Annotations, nil
}

func Save(path string, annotations []Annotation) error {
	f, err := os.Create(path)
	if err != nil {
		return err
	}
	defer f.Close()

	doc := KDLFile{Annotations: annotations}
	return kdl.NewEncoder(f).Encode(doc)
}

// Append adds or updates an annotation in the file
func Append(path string, newAnn Annotation) error {
	anns, err := Load(path)
	if err != nil {
		return err
	}

	// Simple upsert logic
	found := false
	for i, a := range anns {
		if a.ID == newAnn.ID {
			anns[i] = newAnn
			found = true
			break
		}
	}
	if !found {
		anns = append(anns, newAnn)
	}

	return Save(path, anns)
}