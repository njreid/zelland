package kdl

import (
	"os"
	"github.com/sblinch/kdl-go"
)

type Project struct {
	ID          string `kdl:"id,prop"`
	Name        string `kdl:"name,prop,optional"`
	Host        string `kdl:"host,prop"`
	SessionName string `kdl:"session_name,prop"`
	RootPath    string `kdl:"root_path,prop"`
}

type ProjectFile struct {
	Projects []Project `kdl:"project,multiple"`
}

func LoadProjects(path string) ([]Project, error) {
	f, err := os.Open(path)
	if os.IsNotExist(err) {
		return []Project{}, nil
	}
	if err != nil {
		return nil, err
	}
	defer f.Close()

	var doc ProjectFile
	if err := kdl.NewDecoder(f).Decode(&doc); err != nil {
		return nil, err
	}

	return doc.Projects, nil
}

func SaveProjects(path string, projects []Project) error {
	f, err := os.Create(path)
	if err != nil {
		return err
	}
	defer f.Close()

	doc := ProjectFile{Projects: projects}
	return kdl.NewEncoder(f).Encode(doc)
}
