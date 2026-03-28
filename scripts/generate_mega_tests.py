import os

TEST_DIRS = [
    "compiler_project",
    "web_server",
    "game_engine",
    "ml_framework",
    "kernel_test",
    "plugin_system",
    "mega_app"
]

TESTS = {
    "compiler_project/main.nyx": """\
struct Token {
    kind: i32
    lexeme: String
}
fn lex(src: String) -> Token {
    return Token { kind: 1, lexeme: src }
}
fn main() {
    let t = lex("test")
    print(t)
}
""",
    "web_server/main.nyx": """\
struct Response {
    status: i32
    body: String
}
fn handle_request(path: String) -> Response {
    match path {
        1 => return Response { status: 200, body: "OK" }
        _ => return Response { status: 404, body: "Not Found" }
    }
}
fn main() {
    let r = handle_request("/")
    print(r)
}
""",
    "game_engine/main.nyx": """\
struct Entity {
    x: f32
    y: f32
}
fn render_loop() {
    let e = Entity { x: 0.0, y: 0.0 }
    for i in frames {
        print(i)
    }
}
fn main() {
    render_loop()
}
""",
    "ml_framework/main.nyx": """\
struct Tensor {
    shape: i32
}
fn matmul(a: Tensor, b: Tensor) -> Tensor {
    return Tensor { shape: 1 }
}
fn train() {
    let weights = Tensor { shape: 1 }
    let inputs = Tensor { shape: 1 }
    let out = matmul(weights, inputs)
    print(out)
}
fn main() {
    train()
}
""",
    "kernel_test/main.nyx": """\
@no_std
extern "C" fn start() -> i32 {
    let vga = 0xb8000
    asm("cli") : [noop](eax) : [noop](eax)
    return 0
}
""",
    "plugin_system/main.nyx": """\
@plugin("gpu_compute")
extern "C" fn compute_kernel(data: i32) -> i32 {}

fn main() {
    let data = 10
    compute_kernel(data)
}
""",
    "mega_app/main.nyx": """\
struct AppContext {
    db: i32
}
fn run_app() {
    let ctx = AppContext { db: 1 }
    let data = 42
    print(data)
}
fn main() {
    run_app()
}
"""
}

def create_tests():
    for d in TEST_DIRS:
        os.makedirs(os.path.join("tests", d), exist_ok=True)
    
    for path, content in TESTS.items():
        with open(os.path.join("tests", path), "w") as f:
            f.write(content)
            
    runner_script = """#!/bin/bash
set -e
echo "=============================================="
echo "    NYX MASTER TEST RUNNER - 100-YEAR CORE    "
echo "=============================================="

TOTAL=0
PASSED=0

for dir in tests/*/; do
    test_name=$(basename "$dir")
    main_file="${dir}main.nyx"
    
    echo "Running test: $test_name"
    if [ -f "$main_file" ]; then
        NEXT_CMD="cargo run --quiet --manifest-path Cargo.toml -- build $main_file"
        echo "Executing: $NEXT_CMD"
        if eval "$NEXT_CMD" > /dev/null; then
            echo "[PASS] $test_name successfully compiled to IR without crashing the freezing core."
            PASSED=$((PASSED+1))
        else
            echo "[FAIL] $test_name violated architecture constraints."
            exit 1
        fi
        TOTAL=$((TOTAL+1))
    fi
done

echo "=============================================="
echo "Results: $PASSED / $TOTAL massive ecosystem tests passed."
echo "Validation complete."
"""
    with open("tests/run_all_tests", "w") as f:
        f.write(runner_script)
    os.chmod("tests/run_all_tests", 0o755)
    print("Tests generated.")

if __name__ == "__main__":
    create_tests()
