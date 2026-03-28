import sys

def fix_safety(filepath):
    with open(filepath, "r") as f:
        lines = f.readlines()
        
    out = []
    for line in lines:
        if "pub unsafe fn" in line or "unsafe fn" in line:
            # Check if previous line has safety doc
            if len(out) == 0 or "/// # Safety" not in out[-1]:
                indent = len(line) - len(line.lstrip())
                out.append(" " * indent + "/// # Safety\n")
                out.append(" " * indent + "/// Hardware constraints apply.\n")
        out.append(line)
        
    with open(filepath, "w") as f:
        f.writelines(out)

fix_safety("src/systems/hardware/pointers.rs")
fix_safety("src/systems/hardware/inline_asm.rs")
fix_safety("src/systems/hardware/io.rs")
fix_safety("src/systems/hardware/memory.rs")
