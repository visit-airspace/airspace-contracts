package verify_test

import (
	"context"
	"fmt"
	"os"
	"os/exec"
	"path/filepath"
	"slices"
	"strings"
	"testing"
	"time"
)

var steps = []string{"airspace_escrow", "airspace_deposit", "allow", "deny", "test", "fmt", "clippy"}

func TestVerifyStagesFixtures(t *testing.T) {
	output, err, actual := runVerify(t, "", "")
	if err != nil || !slices.Equal(actual, steps) {
		t.Fatalf("make verify: %v, steps = %v, want %v\n%s", err, actual, steps, output)
	}
}

func TestVerifyStopsOnFailure(t *testing.T) {
	for index, step := range steps {
		t.Run(step, func(t *testing.T) {
			output, err, actual := runVerify(t, step, "")
			if err == nil || !slices.Equal(actual, steps[:index+1]) || !strings.Contains(output, "simulated Cargo failure at "+step) {
				t.Fatalf("make verify: %v, steps = %v, want %v\n%s", err, actual, steps[:index+1], output)
			}
		})
	}
	for index, variant := range steps[2:4] {
		t.Run("missing "+variant, func(t *testing.T) {
			output, err, actual := runVerify(t, "", variant)
			want := steps[:index+3]
			if err == nil || !slices.Equal(actual, want) || !strings.Contains(output, "/"+variant+"/test_hook_program.so") {
				t.Fatalf("expected copy failure for %s, got %v, steps = %v, want %v\n%s", variant, err, actual, want, output)
			}
		})
	}
}

func runVerify(t *testing.T, failAt, missingArtifact string) (string, error, []string) {
	t.Helper()
	root := filepath.Join(t.TempDir(), "contracts checkout")
	if err := os.MkdirAll(root, 0o755); err != nil {
		t.Fatal(err)
	}
	makefile, err := os.ReadFile("../Makefile")
	if err != nil {
		t.Fatal(err)
	}
	if err := os.WriteFile(filepath.Join(root, "Makefile"), makefile, 0o644); err != nil {
		t.Fatal(err)
	}
	executable, err := os.Executable()
	if err != nil {
		t.Fatal(err)
	}
	// Invoke this test binary as cargo so the stand-in needs only the Go toolchain.
	if err := os.Symlink(executable, filepath.Join(root, "cargo")); err != nil {
		t.Fatal(err)
	}
	ctx, cancel := context.WithTimeout(context.Background(), 30*time.Second)
	defer cancel()
	cmd := exec.CommandContext(ctx, "make", "-j4", "verify")
	cmd.Dir = root
	cmd.Env = append(os.Environ(),
		"PATH="+root+string(os.PathListSeparator)+os.Getenv("PATH"),
		"VERIFY_FAIL_AT="+failAt,
		"VERIFY_MISSING_ARTIFACT="+missingArtifact,
		"CARGO_TARGET_DIR="+filepath.Join(root, "custom Cargo output"),
	)
	output, runErr := cmd.CombinedOutput()
	if ctx.Err() != nil {
		t.Fatalf("make verify timed out\n%s", output)
	}
	log, err := os.ReadFile(filepath.Join(root, "cargo.log"))
	if err != nil {
		t.Fatalf("read Cargo stand-in log: %v\n%s", err, output)
	}
	return string(output), runErr, strings.Fields(string(log))
}

func TestMain(m *testing.M) {
	if filepath.Base(os.Args[0]) == "cargo" {
		if err := runCargo(os.Args[1:]); err != nil {
			fmt.Fprintln(os.Stderr, err)
			os.Exit(1)
		}
		os.Exit(0)
	}
	os.Exit(m.Run())
}

func runCargo(args []string) error {
	root, err := os.Getwd()
	if err != nil {
		return err
	}
	deploy := filepath.Join(root, "target", "deploy")
	for _, name := range steps {
		var want []string
		var artifact string
		switch name {
		case "airspace_escrow", "airspace_deposit":
			program := strings.ReplaceAll(name, "_", "-")
			want = []string{"build-sbf", "--manifest-path", "programs/" + program + "/Cargo.toml", "--sbf-out-dir", deploy}
			artifact = filepath.Join(deploy, name+".so")
		case "allow", "deny":
			output := filepath.Join(deploy, name)
			want = []string{"build-sbf", "--manifest-path", "tests/test-hook-program/Cargo.toml", "--features", name, "--sbf-out-dir", output}
			artifact = filepath.Join(output, "test_hook_program.so")
		case "test":
			want = []string{"test"}
		case "fmt":
			want = []string{"fmt", "--all", "--check"}
		case "clippy":
			want = []string{"clippy", "--all-targets", "--", "-D", "warnings"}
		}
		if !slices.Equal(args, want) {
			continue
		}
		log, err := os.OpenFile("cargo.log", os.O_CREATE|os.O_APPEND|os.O_WRONLY, 0o644)
		if err != nil {
			return err
		}
		defer log.Close()
		if _, err := fmt.Fprintln(log, name); err != nil {
			return err
		}
		if name == os.Getenv("VERIFY_FAIL_AT") {
			return fmt.Errorf("simulated Cargo failure at %s", name)
		}
		if name == "deny" {
			if err := checkFixture(deploy, "test_hook_allow", "allow"); err != nil {
				return err
			}
		}
		if name == "test" {
			for _, fixture := range steps[:4] {
				filename := fixture
				if fixture == "allow" || fixture == "deny" {
					filename = "test_hook_" + fixture
				}
				if err := checkFixture(deploy, filename, fixture); err != nil {
					return err
				}
			}
		}
		if artifact == "" || name == os.Getenv("VERIFY_MISSING_ARTIFACT") {
			return nil
		}
		if err := os.MkdirAll(filepath.Dir(artifact), 0o755); err != nil {
			return err
		}
		return os.WriteFile(artifact, []byte(name), 0o644)
	}
	return fmt.Errorf("unexpected Cargo arguments: %q", args)
}

func checkFixture(deploy, filename, want string) error {
	content, err := os.ReadFile(filepath.Join(deploy, filename+".so"))
	if err != nil {
		return err
	}
	if string(content) != want {
		return fmt.Errorf("%s fixture contains %q, want %q", filename, content, want)
	}
	return nil
}
