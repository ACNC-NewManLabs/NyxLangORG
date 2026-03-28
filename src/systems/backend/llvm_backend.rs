use crate::systems::ir::nyx_ir::{BinaryOp, IrType, Instruction, Module, StructDef, Value};
use std::collections::HashMap;
use std::collections::{BTreeMap, HashSet};
use std::fmt::Write;

#[derive(Debug, Clone, Copy)]
pub enum Target {
    X86_64,
    FreestandingX86_64,
    AArch64,
    RiscV64,
    Wasm32,
    BrowserJs,
    Bytecode,
    Ast,
}

impl Target {
    pub fn triple(self) -> &'static str {
        match self {
            Target::X86_64 => "x86_64-unknown-linux-gnu",
            Target::FreestandingX86_64 => "x86_64-unknown-none-elf",
            Target::AArch64 => "aarch64-unknown-linux-gnu",
            Target::RiscV64 => "riscv64-unknown-linux-gnu",
            Target::Wasm32 => "wasm32-unknown-unknown",
            Target::BrowserJs => "browser-js",
            Target::Bytecode => "bytecode",
            Target::Ast => "ast",
        }
    }
}

#[derive(Debug, Default)]
pub struct LlvmBackend;

impl LlvmBackend {
    pub fn lower_to_llvm_ir(&self, module: &Module, target: Target) -> Result<String, String> {
        let mut out = String::new();
        let mut strings = StringTable::default();
        collect_string_literals(module, &mut strings);

        out.push_str(&format!("target triple = \"{}\"\n", target.triple()));

        if matches!(target, Target::FreestandingX86_64) {
            // Multiboot2 Magic: 0xE85250D6 (-397250346)
            // Architecture: 0 (i386)
            // Header length: 16 bytes
            // Checksum: -(0xE85250D6 + 0 + 16) = 0x17ADAF1A (397250330)
            out.push_str("@multiboot_header = dso_local constant { i32, i32, i32, i32 } { ");
            out.push_str("i32 -397250346, i32 0, i32 16, i32 397250330 }, section \".multiboot\", align 8\n");
        }

        out.push_str("@.fmt_i64 = private unnamed_addr constant [6 x i8] c\"%lld\\0A\\00\"\n");
        out.push_str("@.fmt_f64 = private unnamed_addr constant [4 x i8] c\"%f\\0A\\00\"\n");
        out.push_str("@.fmt_str = private unnamed_addr constant [4 x i8] c\"%s\\0A\\00\"\n");
        for def in strings.definitions() {
            out.push_str(def.as_str());
        }
        
        if !matches!(target, Target::FreestandingX86_64) {
            out.push_str("declare i32 @printf(ptr noundef, ...)\n");
            out.push_str("declare ptr @malloc(i64)\n");
        }
        for def in &module.structs {
            out.push_str(&format!(
                "%{} = type {{ {} }}\n",
                def.name,
                def.fields
                    .iter()
                    .map(|f| llvm_type_from_ir(&f.ty))
                    .collect::<Vec<_>>()
                    .join(", ")
            ));
        }
        let array_types = collect_array_types(module);
        for arr in &array_types {
            out.push_str(&format!(
                "%{} = type {{ {}*, i64 }}\n",
                arr.name,
                arr.elem_ty
            ));
        }
        out.push('\n');

        let defined: HashSet<&str> = module.functions.iter().map(|f| f.name.as_str()).collect();
        let mut extern_decls: BTreeMap<String, usize> = BTreeMap::new();
        for func in &module.functions {
            for instr in &func.instructions {
                if let Instruction::Call { callee, args, .. } = instr {
                    if !defined.contains(callee.as_str()) && callee != "printf" {
                        let arity = args.len();
                        extern_decls
                            .entry(callee.clone())
                            .and_modify(|cur| *cur = (*cur).max(arity))
                            .or_insert(arity);
                    }
                }
            }
        }

        for (name, _arity) in extern_decls {
            out.push_str(&format!(
                "declare i64 {}(...)\n",
                llvm_global_ident(&name)
            ));
        }
        if !out.ends_with('\n') {
            out.push('\n');
        }

        for func in &module.functions {
            let is_main = func.name == "main";
            let ret_ty = if is_main { "i32" } else { "i64" };
            let mut param_defs = Vec::new();
            for (idx, p) in func.params.iter().enumerate() {
                let llvm_ty = llvm_type_from_ir(&p.ty);
                param_defs.push(format!("{llvm_ty} %arg{idx}"));
            }
            let params = param_defs.join(", ");
            out.push_str(&format!(
                "define {ret_ty} {}({params}) {{\n",
                llvm_global_ident(&func.name),
            ));
            out.push_str("entry:\n");

            let mut ids = 0usize;
            let mut locals: HashMap<String, TypedValue> = HashMap::new();
            let mut labels: HashMap<String, String> = HashMap::new();
            let struct_map = build_struct_map(&module.structs);

            for (idx, p) in func.params.iter().enumerate() {
                let llvm_ty = llvm_type_from_ir(&p.ty);
                let ty = llvm_type_to_llvmt(llvm_ty)?;
                locals.insert(
                    p.name.clone(),
                    TypedValue {
                        repr: format!("%arg{idx}"),
                        ty,
                    },
                );
            }

            for instr in &func.instructions {
                if let Instruction::Label(label) = instr {
                    let next = labels.len();
                    labels
                        .entry(label.clone())
                        .or_insert_with(|| llvm_block_ident(label, next));
                }
            }

            for instr in &func.instructions {
                match instr {
                    Instruction::Let { name, value } => {
                        let rhs = value_to_llvm(value, &locals, &strings)?;
                        if matches!(rhs.ty, LlvmType::Struct(_)) {
                            locals.insert(name.clone(), rhs);
                            continue;
                        }
                        let reg = format!("%v{ids}");
                        ids += 1;
                        match rhs.ty {
                            LlvmType::I64 => {
                                out.push_str(&format!("  {reg} = add i64 0, {}\n", rhs.repr));
                            }
                            LlvmType::F64 => {
                                out.push_str(&format!(
                                    "  {reg} = fadd double 0.0, {}\n",
                                    rhs.repr
                                ));
                            }
                            LlvmType::Ptr => {
                                out.push_str(&format!(
                                    "  {reg} = bitcast ptr {} to ptr\n",
                                    rhs.repr
                                ));
                            }
                            LlvmType::Struct(_)
                            | LlvmType::F32
                            | LlvmType::Vector(_, _) => {}
                        }
                        locals.insert(
                            name.clone(),
                            TypedValue {
                                repr: reg,
                                ty: rhs.ty,
                            },
                        );
                    }
                    Instruction::Binary { dst, op, lhs, rhs } => {
                        let lhs_s = value_to_llvm(lhs, &locals, &strings)?;
                        let rhs_s = value_to_llvm(rhs, &locals, &strings)?;
                        match op {
                            BinaryOp::Eq
                            | BinaryOp::Ne
                            | BinaryOp::Lt
                            | BinaryOp::Le
                            | BinaryOp::Gt
                            | BinaryOp::Ge => {
                                let (cmp_reg, cmp_line) =
                                    emit_compare(op, &lhs_s, &rhs_s, &mut ids)?;
                                out.push_str(&cmp_line);
                                let reg = format!("%v{ids}");
                                ids += 1;
                                out.push_str(&format!("  {reg} = zext i1 {cmp_reg} to i64\n"));
                                locals.insert(
                                    dst.clone(),
                                    TypedValue {
                                        repr: reg,
                                        ty: LlvmType::I64,
                                    },
                                );
                            }
                            BinaryOp::And | BinaryOp::Or => {
                                if lhs_s.ty != LlvmType::I64 || rhs_s.ty != LlvmType::I64 {
                                    return Err(
                                        "logical operations require integer/boolean operands"
                                            .into(),
                                    );
                                }
                                let lhs_bool = format!("%v{ids}");
                                ids += 1;
                                out.push_str(&format!(
                                    "  {lhs_bool} = icmp ne i64 {}, 0\n",
                                    lhs_s.repr
                                ));
                                let rhs_bool = format!("%v{ids}");
                                ids += 1;
                                out.push_str(&format!(
                                    "  {rhs_bool} = icmp ne i64 {}, 0\n",
                                    rhs_s.repr
                                ));
                                let bool_reg = format!("%v{ids}");
                                ids += 1;
                                let llvm_op = match op {
                                    BinaryOp::And => "and",
                                    BinaryOp::Or => "or",
                                    _ => return Err("invalid logical op".into()),
                                };
                                out.push_str(&format!(
                                    "  {bool_reg} = {llvm_op} i1 {lhs_bool}, {rhs_bool}\n"
                                ));
                                let reg = format!("%v{ids}");
                                ids += 1;
                                out.push_str(&format!("  {reg} = zext i1 {bool_reg} to i64\n"));
                                locals.insert(
                                    dst.clone(),
                                    TypedValue {
                                        repr: reg,
                                        ty: LlvmType::I64,
                                    },
                                );
                            }
                            BinaryOp::Not => {
                                if rhs_s.ty != LlvmType::I64 {
                                    return Err(
                                        "logical not requires integer/boolean operand".into(),
                                    );
                                }
                                let bool_reg = format!("%v{ids}");
                                ids += 1;
                                out.push_str(&format!(
                                    "  {bool_reg} = icmp eq i64 {}, 0\n",
                                    rhs_s.repr
                                ));
                                let reg = format!("%v{ids}");
                                ids += 1;
                                out.push_str(&format!("  {reg} = zext i1 {bool_reg} to i64\n"));
                                locals.insert(
                                    dst.clone(),
                                    TypedValue {
                                        repr: reg,
                                        ty: LlvmType::I64,
                                    },
                                );
                            }
                            _ => {
                                let reg = format!("%v{ids}");
                                ids += 1;
                                let (line, ty) =
                                    emit_binary(op, &lhs_s, &rhs_s, &reg).map_err(|e| {
                                        format!("unsupported binary op during LLVM lowering: {e}")
                                    })?;
                                out.push_str(&line);
                                locals.insert(
                                    dst.clone(),
                                    TypedValue {
                                        repr: reg,
                                        ty,
                                    },
                                );
                            }
                        }
                    }
                    Instruction::Print { value } => {
                        let val = value_to_llvm(value, &locals, &strings)?;
                        let fmt = match val.ty {
                            LlvmType::I64 => "@.fmt_i64",
                            LlvmType::F64 => "@.fmt_f64",
                            LlvmType::Ptr => "@.fmt_str",
                            LlvmType::Struct(_)
                            | LlvmType::F32
                            | LlvmType::Vector(_, _) => {
                                return Err("cannot print non-scalar values".into());
                            }
                        };
                        let arg = match val.ty {
                            LlvmType::I64 => format!("i64 noundef {}", val.repr),
                            LlvmType::F64 => format!("double noundef {}", val.repr),
                            LlvmType::Ptr => format!("ptr noundef {}", val.repr),
                            LlvmType::Struct(_)
                            | LlvmType::F32
                            | LlvmType::Vector(_, _) => unreachable!(),
                        };
                        out.push_str(&format!(
                            "  %p{ids} = call i32 (ptr, ...) @printf(ptr noundef {fmt}, {arg})\n"
                        ));
                        ids += 1;
                    }
                    Instruction::Return { value } => {
                        if is_main {
                            let code = match value {
                                Some(v) => value_to_llvm(v, &locals, &strings)?,
                                None => TypedValue {
                                    repr: "0".to_string(),
                                    ty: LlvmType::I64,
                                },
                            };
                            if code.ty != LlvmType::I64 {
                                return Err("main must return an integer value".into());
                            }
                            out.push_str(&format!("  ret i32 {}\n", code.repr));
                        } else {
                            let code = match value {
                                Some(v) => value_to_llvm(v, &locals, &strings)?,
                                None => TypedValue {
                                    repr: "0".to_string(),
                                    ty: LlvmType::I64,
                                },
                            };
                            if code.ty != LlvmType::I64 {
                                return Err("function must return an integer value".into());
                            }
                            out.push_str(&format!("  ret i64 {}\n", code.repr));
                        }
                    }
                    Instruction::Label(lbl) => {
                        out.push_str(&format!("{}:\n", label_name(lbl, &labels)));
                    }
                    Instruction::Jump(label) => {
                        // Attach loop vectorization metadata to back-edges (identified by label patterns)
                        if label.contains("cond") || label.contains("loop") {
                            out.push_str(&format!("  br label %{}, !llvm.loop !0\n", label_name(label, &labels)));
                        } else {
                            out.push_str(&format!("  br label %{}\n", label_name(label, &labels)));
                        }
                    }
                    Instruction::Branch {
                        cond,
                        then_label,
                        else_label,
                    } => {
                        let cv = value_to_llvm(cond, &locals, &strings)?;
                        if cv.ty != LlvmType::I64 {
                            return Err("branch condition must be integer/boolean".into());
                        }
                        let cond_reg = format!("%v{ids}");
                        ids += 1;
                        out.push_str(&format!(
                            "  {cond_reg} = icmp ne i64 {}, 0\n",
                            cv.repr
                        ));

                        out.push_str(&format!(
                            "  br i1 {cond_reg}, label %{}, label %{}\n",
                            label_name(then_label, &labels),
                            label_name(else_label, &labels)
                        ));
                    }
                    Instruction::Call { dst, callee, args } => {
                        if matches!(target, Target::FreestandingX86_64) {
                            if callee == "std::kernel::ports::outb" && args.len() == 2 {
                                let port = value_to_llvm(&args[0], &locals, &strings)?;
                                let val = value_to_llvm(&args[1], &locals, &strings)?;
                                
                                let trunc_port = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {trunc_port} = trunc i64 {} to i16\n", port.repr));
                                let trunc_val = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {trunc_val} = trunc i64 {} to i8\n", val.repr));
                                
                                out.push_str(&format!("  call void asm sideeffect \"outb $0, $1\", \"{{al}},{{dx}},~{{dirflag}},~{{fpsr}},~{{flags}}\"(i8 {trunc_val}, i16 {trunc_port})\n"));
                                
                                locals.insert(
                                    dst.clone(),
                                    TypedValue { repr: "0".to_string(), ty: LlvmType::I64 },
                                );
                                continue;
                            } else if callee == "std::kernel::ports::inb" && args.len() == 1 {
                                let port = value_to_llvm(&args[0], &locals, &strings)?;
                                
                                let trunc_port = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {trunc_port} = trunc i64 {} to i16\n", port.repr));
                                
                                let res_i8 = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {res_i8} = call i8 asm sideeffect \"inb $1, $0\", \"={{al}},{{dx}},~{{dirflag}},~{{fpsr}},~{{flags}}\"(i16 {trunc_port})\n"));
                                
                                let reg = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {reg} = zext i8 {res_i8} to i64\n"));
                                
                                locals.insert(
                                    dst.clone(),
                                    TypedValue { repr: reg, ty: LlvmType::I64 },
                                );
                                continue;
                            } else if callee == "std::kernel::memory::write_byte" && args.len() == 2 {
                                let addr = value_to_llvm(&args[0], &locals, &strings)?;
                                let val = value_to_llvm(&args[1], &locals, &strings)?;
                                
                                let ptr = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {ptr} = inttoptr i64 {} to i8*\n", addr.repr));
                                let trunc_val = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {trunc_val} = trunc i64 {} to i8\n", val.repr));
                                
                                out.push_str(&format!("  store volatile i8 {trunc_val}, i8* {ptr}\n"));
                                
                                locals.insert(
                                    dst.clone(),
                                    TypedValue { repr: "0".to_string(), ty: LlvmType::I64 },
                                );
                                continue;
                            } else if callee == "std::kernel::memory::read_byte" && args.len() == 1 {
                                let addr = value_to_llvm(&args[0], &locals, &strings)?;
                                
                                let ptr = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {ptr} = inttoptr i64 {} to i8*\n", addr.repr));
                                
                                let res_i8 = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {res_i8} = load volatile i8, i8* {ptr}\n"));
                                
                                let reg = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {reg} = zext i8 {res_i8} to i64\n"));
                                
                                locals.insert(
                                    dst.clone(),
                                    TypedValue { repr: reg, ty: LlvmType::I64 },
                                );
                                continue;
                            } else if callee == "std::kernel::memory::write_u64" && args.len() == 2 {
                                let addr = value_to_llvm(&args[0], &locals, &strings)?;
                                let val = value_to_llvm(&args[1], &locals, &strings)?;
                                
                                let ptr = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {ptr} = inttoptr i64 {} to i64*\n", addr.repr));
                                
                                out.push_str(&format!("  store volatile i64 {}, i64* {ptr}\n", val.repr));
                                
                                locals.insert(
                                    dst.clone(),
                                    TypedValue { repr: "0".to_string(), ty: LlvmType::I64 },
                                );
                                continue;
                            } else if callee == "std::kernel::memory::read_u64" && args.len() == 1 {
                                let addr = value_to_llvm(&args[0], &locals, &strings)?;
                                
                                let ptr = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {ptr} = inttoptr i64 {} to i64*\n", addr.repr));
                                
                                let reg = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {reg} = load volatile i64, i64* {ptr}\n"));
                                
                                locals.insert(
                                    dst.clone(),
                                    TypedValue { repr: reg, ty: LlvmType::I64 },
                                );
                                continue;
                            } else if callee == "std::kernel::cpu::cli" {
                                out.push_str("  call void asm sideeffect \"cli\", \"~{dirflag},~{fpsr},~{flags}\"()\n");
                                continue;
                            } else if callee == "std::kernel::cpu::sti" {
                                out.push_str("  call void asm sideeffect \"sti\", \"~{dirflag},~{fpsr},~{flags}\"()\n");
                                continue;
                            } else if callee == "std::kernel::cpu::hlt" {
                                out.push_str("  call void asm sideeffect \"hlt\", \"~{dirflag},~{fpsr},~{flags}\"()\n");
                                continue;
                            } else if callee == "std::kernel::cpu::context_switch" && args.len() == 2 {
                                let old_ptr = value_to_llvm(&args[0], &locals, &strings)?;
                                let new_ptr = value_to_llvm(&args[1], &locals, &strings)?;
                                
                                // context_switch(*u64 old_stack_ptr, u64 new_stack_val)
                                // We use a complex asm string to save/restore all callee-saved registers
                                out.push_str(&format!(
                                    "  call void asm sideeffect \"pushq %rbx; pushq %rbp; pushq %r12; pushq %r13; pushq %r14; pushq %r15; movq %rsp, $0; movq $1, %rsp; popq %r15; popq %r14; popq %r13; popq %r12; popq %rbp; popq %rbx\", \"=*m,r,~{{dirflag}},~{{fpsr}},~{{flags}}\"(i64* {}, i64 {})\n",
                                    old_ptr.repr, new_ptr.repr
                                ));
                                continue;
                            } else if callee == "std::kernel::cpu::lidt" && args.len() == 1 {
                                let addr = value_to_llvm(&args[0], &locals, &strings)?;
                                out.push_str(&format!("  call void asm sideeffect \"lidt ($0)\", \"r,~{{dirflag}},~{{fpsr}},~{{flags}}\"(i64 {})\n", addr.repr));
                                continue;
                            } else if callee == "len" && args.len() == 1 {
                                let val = value_to_llvm(&args[0], &locals, &strings)?;
                                let reg = format!("%v{ids}"); ids += 1;
                                match &val.ty {
                                    LlvmType::Struct(name) if name.starts_with("nyx_array_") => {
                                        out.push_str(&format!("  {reg} = extractvalue %{} {}, 1\n", name, val.repr));
                                    }
                                    _ => {
                                        let ptr_val = if val.ty == LlvmType::I64 {
                                            let p = format!("%v{ids}"); ids += 1;
                                            out.push_str(&format!("  {p} = inttoptr i64 {} to ptr\n", val.repr));
                                            p
                                        } else {
                                            val.repr.clone()
                                        };
                                        
                                        // String length logic (naive strlen in IR)
                                        let loop_label = format!("len_loop_{ids}");
                                        let end_label = format!("len_end_{ids}");
                                        let phi_reg = format!("%phi_{ids}");
                                        let char_reg = format!("%ch_{ids}");
                                        let cond_reg = format!("%cond_{ids}");
                                        let next_reg = format!("%next_{ids}");
                                        ids += 1;
                                        
                                        out.push_str(&format!("  br label %{loop_label}\n"));
                                        out.push_str(&format!("{loop_label}:\n"));
                                        out.push_str(&format!("  {phi_reg} = phi i64 [ 0, %entry ], [ {next_reg}, %{loop_label} ]\n"));
                                        out.push_str(&format!("  {char_reg} = getelementptr i8, ptr {ptr_val}, i64 {phi_reg}\n"));
                                        let load_reg = format!("%lch_{ids}"); ids += 1;
                                        out.push_str(&format!("  {load_reg} = load i8, ptr {char_reg}\n"));
                                        out.push_str(&format!("  {cond_reg} = icmp ne i8 {load_reg}, 0\n"));
                                        out.push_str(&format!("  {next_reg} = add i64 {phi_reg}, 1\n"));
                                        out.push_str(&format!("  br i1 {cond_reg}, label %{loop_label}, label %{end_label}\n"));
                                        out.push_str(&format!("{end_label}:\n"));
                                        out.push_str(&format!("  {reg} = add i64 {phi_reg}, 0\n"));
                                    }
                                }
                                locals.insert(dst.clone(), TypedValue { repr: reg, ty: LlvmType::I64 });
                                continue;
                            } else if callee == "char_at" && args.len() == 2 {
                                let str_val = value_to_llvm(&args[0], &locals, &strings)?;
                                let idx = value_to_llvm(&args[1], &locals, &strings)?;
                                
                                let ptr_val = if str_val.ty == LlvmType::I64 {
                                    let p = format!("%v{ids}"); ids += 1;
                                    out.push_str(&format!("  {p} = inttoptr i64 {} to ptr\n", str_val.repr));
                                    p
                                } else {
                                    str_val.repr.clone()
                                };
                                
                                let reg = format!("%v{ids}"); ids += 1;
                                let ptr = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {ptr} = getelementptr i8, ptr {ptr_val}, i64 {}\n", idx.repr));
                                out.push_str(&format!("  {reg} = load i8, ptr {ptr}\n"));
                                let res = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {res} = zext i8 {reg} to i64\n"));
                                locals.insert(dst.clone(), TypedValue { repr: res, ty: LlvmType::I64 });
                                continue;
                            } else if callee == "std::simd::f32x4_load" && args.len() == 1 {
                                let addr = value_to_llvm(&args[0], &locals, &strings)?;
                                let reg = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {reg} = load <4 x float>, ptr {}\n", addr.repr));
                                locals.insert(dst.clone(), TypedValue { 
                                    repr: reg, 
                                    ty: LlvmType::Vector(Box::new(LlvmType::F32), 4) 
                                });
                                continue;
                            } else if callee == "std::simd::f32x4_store" && args.len() == 2 {
                                let addr = value_to_llvm(&args[0], &locals, &strings)?;
                                let val = value_to_llvm(&args[1], &locals, &strings)?;
                                out.push_str(&format!("  store <4 x float> {}, ptr {}\n", val.repr, addr.repr));
                                continue;
                            } else if callee == "std::simd::f32x4_add" && args.len() == 2 {
                                let v1 = value_to_llvm(&args[0], &locals, &strings)?;
                                let v2 = value_to_llvm(&args[1], &locals, &strings)?;
                                let reg = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {reg} = fadd <4 x float> {}, {}\n", v1.repr, v2.repr));
                                locals.insert(dst.clone(), TypedValue { 
                                    repr: reg, 
                                    ty: LlvmType::Vector(Box::new(LlvmType::F32), 4) 
                                });
                                continue;
                            } else if callee == "std::simd::f32x4_sub" && args.len() == 2 {
                                let v1 = value_to_llvm(&args[0], &locals, &strings)?;
                                let v2 = value_to_llvm(&args[1], &locals, &strings)?;
                                let reg = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {reg} = fsub <4 x float> {}, {}\n", v1.repr, v2.repr));
                                locals.insert(dst.clone(), TypedValue { 
                                    repr: reg, 
                                    ty: LlvmType::Vector(Box::new(LlvmType::F32), 4) 
                                });
                                continue;
                            } else if callee == "std::simd::f32x4_mul" && args.len() == 2 {
                                let v1 = value_to_llvm(&args[0], &locals, &strings)?;
                                let v2 = value_to_llvm(&args[1], &locals, &strings)?;
                                let reg = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {reg} = fmul <4 x float> {}, {}\n", v1.repr, v2.repr));
                                locals.insert(dst.clone(), TypedValue { 
                                    repr: reg, 
                                    ty: LlvmType::Vector(Box::new(LlvmType::F32), 4) 
                                });
                                continue;
                            } else if callee == "std::kernel::memory::fence" {
                                out.push_str("  fence seq_cst\n");
                                continue;
                            } else if callee == "std::kernel::cpu::rdtsc" {
                                let reg = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {reg} = call i64 asm sideeffect \"rdtsc; shl $32, %%rdx; or %%rdx, %%rax\", \"={{rax}},~{{rdx}},~{{dirflag}},~{{fpsr}},~{{flags}}\"()\n"));
                                locals.insert(dst.clone(), TypedValue { repr: reg, ty: LlvmType::I64 });
                                continue;
                            } else if callee == "std::kernel::cpu::cpuid_reg" && args.len() == 3 {
                                let leaf = value_to_llvm(&args[0], &locals, &strings)?;
                                let subleaf = value_to_llvm(&args[1], &locals, &strings)?;
                                let _reg_idx = value_to_llvm(&args[2], &locals, &strings)?;
                                
                                let struct_reg = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {struct_reg} = call {{i32, i32, i32, i32}} asm sideeffect \"cpuid\", \"={{ax}},={{bx}},={{cx}},={{dx}},{{ax}},{{cx}},~{{dirflag}},~{{fpsr}},~{{flags}}\"(i32 {}, i32 {})\n", leaf.repr, subleaf.repr));
                                
                                let res_32 = format!("%v{ids}"); ids += 1;
                                // Map reg_idx (0,1,2,3) to index in struct
                                let idx_str = match &args[2] {
                                    Value::Int(0) => "0",
                                    Value::Int(1) => "1",
                                    Value::Int(2) => "2",
                                    Value::Int(3) => "3",
                                    _ => "0", // Fallback
                                };
                                out.push_str(&format!("  {res_32} = extractvalue {{i32, i32, i32, i32}} {struct_reg}, {}\n", idx_str));
                                let res_64 = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {res_64} = zext i32 {res_32} to i64\n"));
                                locals.insert(dst.clone(), TypedValue { repr: res_64, ty: LlvmType::I64 });
                                continue;
                            } else if callee == "std::simd::f32x4_splat" && args.len() == 1 {
                                let val = value_to_llvm(&args[0], &locals, &strings)?;
                                let reg = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {reg} = insertelement <4 x float> undef, float {}, i32 0\n", val.repr));
                                let reg2 = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!("  {reg2} = shufflevector <4 x float> {reg}, <4 x float> undef, <4 x i32> zeroinitializer\n"));
                                locals.insert(dst.clone(), TypedValue { 
                                    repr: reg2, 
                                    ty: LlvmType::Vector(Box::new(LlvmType::F32), 4) 
                                });
                                continue;
                            } else if callee == "std::os::linux::syscall" && args.len() == 7 {
                                let sys_no = value_to_llvm(&args[0], &locals, &strings)?;
                                let a1 = value_to_llvm(&args[1], &locals, &strings)?;
                                let a2 = value_to_llvm(&args[2], &locals, &strings)?;
                                let a3 = value_to_llvm(&args[3], &locals, &strings)?;
                                let a4 = value_to_llvm(&args[4], &locals, &strings)?;
                                let a5 = value_to_llvm(&args[5], &locals, &strings)?;
                                let a6 = value_to_llvm(&args[6], &locals, &strings)?;
                                
                                let reg = format!("%v{ids}"); ids += 1;
                                out.push_str(&format!(
                                    "  {} = call i64 asm sideeffect \"syscall\", \"={{rax}},{{rax}},{{rdi}},{{rsi}},{{rdx}},{{r10}},{{r8}},{{r9}},~{{rcx}},~{{r11}},~{{memory}},~{{dirflag}},~{{fpsr}},~{{flags}}\"(i64 {}, i64 {}, i64 {}, i64 {}, i64 {}, i64 {}, i64 {})\n",
                                    reg, sys_no.repr, a1.repr, a2.repr, a3.repr, a4.repr, a5.repr, a6.repr
                                ));
                                locals.insert(dst.clone(), TypedValue { repr: reg, ty: LlvmType::I64 });
                                continue;
                            }
                        }

                        let arg_strs: Vec<String> = args
                            .iter()
                            .map(|a| {
                                let v = value_to_llvm(a, &locals, &strings)?;
                                Ok(match v.ty {
                                    LlvmType::I64 => format!("i64 {}", v.repr),
                                    LlvmType::F64 => format!("double {}", v.repr),
                                    LlvmType::F32
                                    | LlvmType::Vector(_, _) => {
                                        format!("{} {}", llvm_type(&v.ty), v.repr)
                                    }
                                    LlvmType::Ptr => format!("ptr {}", v.repr),
                                    LlvmType::Struct(_) => {
                                        let ty = llvm_type_string(&v.ty);
                                        format!("{ty} {}", v.repr)
                                    }
                                })
                            })
                            .collect::<Result<Vec<_>, String>>()?;
                        let reg = format!("%v{ids}");
                        ids += 1;
                        out.push_str(&format!(
                            "  {reg} = call i64 {}({})\n",
                            llvm_global_ident(callee),
                            arg_strs.join(", ")
                        ));
                        locals.insert(
                            dst.clone(),
                            TypedValue {
                                repr: reg,
                                ty: LlvmType::I64,
                            },
                        );
                    }
                    Instruction::StructInit {
                        dst,
                        struct_name,
                        fields,
                    } => {
                        let def = struct_map.get(struct_name).ok_or_else(|| {
                            format!("unknown struct '{}' during LLVM lowering", struct_name)
                        })?;
                        let mut field_map: HashMap<String, Value> = HashMap::new();
                        for (name, val) in fields {
                            field_map.insert(name.clone(), val.clone());
                        }
                        let mut cur = format!("undef");
                        for (idx, field) in def.fields.iter().enumerate() {
                            let val = field_map.get(&field.name).ok_or_else(|| {
                                format!(
                                    "missing field '{}' in struct literal '{}'",
                                    field.name, struct_name
                                )
                            })?;
                            let v = value_to_llvm(val, &locals, &strings)?;
                            let expected = llvm_type_from_ir(&field.ty);
                            let actual = llvm_type_string(&v.ty);
                            if expected != actual {
                                return Err(format!(
                                    "type mismatch for field '{}.{}': expected {}, got {}",
                                    struct_name, field.name, expected, actual
                                ));
                            }
                            let reg = format!("%v{ids}");
                            ids += 1;
                            out.push_str(&format!(
                                "  {reg} = insertvalue %{} {cur}, {} {}, {idx}\n",
                                struct_name, actual, v.repr
                            ));
                            cur = reg;
                        }
                        locals.insert(
                            dst.clone(),
                            TypedValue {
                                repr: cur,
                                ty: LlvmType::Struct(struct_name.clone()),
                            },
                        );
                    }
                    Instruction::StructGet {
                        dst,
                        struct_name,
                        base,
                        field,
                    } => {
                        let def = struct_map.get(struct_name).ok_or_else(|| {
                            format!("unknown struct '{}' during LLVM lowering", struct_name)
                        })?;
                        let base_val = locals.get(base).ok_or_else(|| {
                            format!("undefined local '{base}' during LLVM lowering")
                        })?;
                        if base_val.ty != LlvmType::Struct(struct_name.clone()) {
                            return Err(format!(
                                "field access on non-struct value '{}' during LLVM lowering",
                                base
                            ));
                        }
                        let idx = def
                            .fields
                            .iter()
                            .position(|f| f.name == *field)
                            .ok_or_else(|| {
                                format!(
                                    "unknown field '{}' in struct '{}' during LLVM lowering",
                                    field, struct_name
                                )
                            })?;
                        let field_ty = llvm_type_from_ir(&def.fields[idx].ty);
                        let reg = format!("%v{ids}");
                        ids += 1;
                        out.push_str(&format!(
                            "  {reg} = extractvalue %{} {}, {idx}\n",
                            struct_name, base_val.repr
                        ));
                        locals.insert(
                            dst.clone(),
                            TypedValue {
                                repr: reg,
                                ty: llvm_type_to_llvmt(field_ty)?,
                            },
                        );
                    }
                    Instruction::ArrayInit { dst, elem_ty, len } => {
                        let elem_ll = llvm_type_from_ir(elem_ty);
                        let arr_name = array_struct_name(&elem_ll);
                        let len_val = value_to_llvm(len, &locals, &strings)?;
                        if len_val.ty != LlvmType::I64 {
                            return Err("array length must be integer".into());
                        }
                        let size_ptr = format!("%v{ids}");
                        ids += 1;
                        out.push_str(&format!(
                            "  {size_ptr} = getelementptr {elem_ll}, ptr null, i64 1\n"
                        ));
                        let size_i64 = format!("%v{ids}");
                        ids += 1;
                        out.push_str(&format!(
                            "  {size_i64} = ptrtoint ptr {size_ptr} to i64\n"
                        ));
                        let bytes = format!("%v{ids}");
                        ids += 1;
                        out.push_str(&format!(
                            "  {bytes} = mul i64 {size_i64}, {}\n",
                            len_val.repr
                        ));
                        let raw = format!("%v{ids}");
                        ids += 1;
                        out.push_str(&format!("  {raw} = call ptr @malloc(i64 {bytes})\n"));
                        let typed = format!("%v{ids}");
                        ids += 1;
                        out.push_str(&format!(
                            "  {typed} = bitcast ptr {raw} to {elem_ll}*\n"
                        ));
                        let arr = format!("%v{ids}");
                        ids += 1;
                        out.push_str(&format!(
                            "  {arr} = insertvalue %{} undef, {elem_ll}* {typed}, 0\n",
                            arr_name
                        ));
                        let arr2 = format!("%v{ids}");
                        ids += 1;
                        out.push_str(&format!(
                            "  {arr2} = insertvalue %{} {arr}, i64 {}, 1\n",
                            arr_name, len_val.repr
                        ));
                        locals.insert(
                            dst.clone(),
                            TypedValue {
                                repr: arr2,
                                ty: LlvmType::Struct(arr_name),
                            },
                        );
                    }
                    Instruction::ArraySet {
                        base,
                        elem_ty,
                        index,
                        value,
                    } => {
                        let elem_ll = llvm_type_from_ir(elem_ty);
                        let arr_name = array_struct_name(&elem_ll);
                        let base_val = locals.get(base).ok_or_else(|| {
                            format!("undefined local '{base}' during LLVM lowering")
                        })?;
                        if base_val.ty != LlvmType::Struct(arr_name.clone()) {
                            return Err("array set on non-array value".into());
                        }
                        let idx_val = value_to_llvm(index, &locals, &strings)?;
                        if idx_val.ty != LlvmType::I64 {
                            return Err("array index must be integer".into());
                        }
                        let val = value_to_llvm(value, &locals, &strings)?;
                        let elem_ptr = format!("%v{ids}");
                        ids += 1;
                        out.push_str(&format!(
                            "  {elem_ptr} = extractvalue %{} {}, 0\n",
                            arr_name, base_val.repr
                        ));
                        let gep = format!("%v{ids}");
                        ids += 1;
                        out.push_str(&format!(
                            "  {gep} = getelementptr {elem_ll}, {elem_ll}* {elem_ptr}, i64 {}\n",
                            idx_val.repr
                        ));
                        let expect = llvm_type_from_ir(elem_ty);
                        let actual = llvm_type_string(&val.ty);
                        if expect != actual {
                            return Err("array element type mismatch".into());
                        }
                        out.push_str(&format!(
                            "  store {elem_ll} {}, {elem_ll}* {gep}\n",
                            val.repr
                        ));
                    }
                    Instruction::ArrayGet {
                        dst,
                        elem_ty,
                        base,
                        index,
                    } => {
                        let elem_ll = llvm_type_from_ir(elem_ty);
                        let arr_name = array_struct_name(&elem_ll);
                        let base_val = locals.get(base).ok_or_else(|| {
                            format!("undefined local '{base}' during LLVM lowering")
                        })?;
                        if base_val.ty != LlvmType::Struct(arr_name.clone()) {
                            return Err("array get on non-array value".into());
                        }
                        let idx_val = value_to_llvm(index, &locals, &strings)?;
                        if idx_val.ty != LlvmType::I64 {
                            return Err("array index must be integer".into());
                        }
                        let elem_ptr = format!("%v{ids}");
                        ids += 1;
                        out.push_str(&format!(
                            "  {elem_ptr} = extractvalue %{} {}, 0\n",
                            arr_name, base_val.repr
                        ));
                        let gep = format!("%v{ids}");
                        ids += 1;
                        out.push_str(&format!(
                            "  {gep} = getelementptr {elem_ll}, {elem_ll}* {elem_ptr}, i64 {}\n",
                            idx_val.repr
                        ));
                        let reg = format!("%v{ids}");
                        ids += 1;
                        out.push_str(&format!(
                            "  {reg} = load {elem_ll}, {elem_ll}* {gep}\n"
                        ));
                        locals.insert(
                            dst.clone(),
                            TypedValue {
                                repr: reg,
                                ty: llvm_type_to_llvmt(elem_ll)?,
                            },
                        );
                    }
                    Instruction::InlineAsm {
                        code,
                        outputs,
                        inputs,
                    } => {
                        let mut constraints: Vec<String> = Vec::new();
                        for out in outputs {
                            let c = match &out.reg {
                                Some(r) => format!("={{{r}}}"),
                                None => "=r".to_string(),
                            };
                            constraints.push(c);
                        }

                        let mut input_args: Vec<String> = Vec::new();
                        for inp in inputs {
                            let v = value_to_llvm(&inp.value, &locals, &strings)?;
                            let c = match &inp.reg {
                                Some(r) => format!("{{{r}}}"),
                                None => "r".to_string(),
                            };
                            constraints.push(c);
                            input_args.push(format!("{} {}", llvm_type(&v.ty), v.repr));
                        }

                        let asm_code = escape_llvm_string(code);
                        let constraint_str = constraints.join(", ");
                        let args = input_args.join(", ");

                        if outputs.is_empty() {
                            if args.is_empty() {
                                out.push_str(&format!(
                                    "  call void asm sideeffect \"{asm_code}\", \"{constraint_str}\"()\n"
                                ));
                            } else {
                                out.push_str(&format!(
                                    "  call void asm sideeffect \"{asm_code}\", \"{constraint_str}\"({args})\n"
                                ));
                            }
                        } else if outputs.len() == 1 {
                            let reg = format!("%v{ids}");
                            ids += 1;
                            out.push_str(&format!(
                                "  {reg} = call i64 asm sideeffect \"{asm_code}\", \"{constraint_str}\"({args})\n"
                            ));
                            locals.insert(
                                outputs[0].name.clone(),
                                TypedValue {
                                    repr: reg,
                                    ty: LlvmType::I64,
                                },
                            );
                        } else {
                            let struct_ty = format!(
                                "{{ {} }}",
                                std::iter::repeat("i64")
                                    .take(outputs.len())
                                    .collect::<Vec<_>>()
                                    .join(", ")
                            );
                            let reg = format!("%v{ids}");
                            ids += 1;
                            out.push_str(&format!(
                                "  {reg} = call {struct_ty} asm sideeffect \"{asm_code}\", \"{constraint_str}\"({args})\n"
                            ));
                            for (idx, outp) in outputs.iter().enumerate() {
                                let ext = format!("%v{ids}");
                                ids += 1;
                                out.push_str(&format!(
                                    "  {ext} = extractvalue {struct_ty} {reg}, {idx}\n"
                                ));
                                locals.insert(
                                    outp.name.clone(),
                                    TypedValue {
                                        repr: ext,
                                        ty: LlvmType::I64,
                                    },
                                );
                            }
                        }
                    }
                }
            }

            if !out.trim_end().ends_with("ret i64 0") && !out.trim_end().ends_with("ret i32 0") && !out.trim_end().ends_with("ret void") && !out.trim_end().ends_with("ret i8 0") {
                let last_line = out.trim_end().lines().last().unwrap_or("");
                if !last_line.contains("ret ") && !last_line.contains("br ") && !last_line.contains("switch ") {
                    if is_main {
                        out.push_str("  ret i32 0\n");
                    } else {
                        out.push_str("  ret i64 0\n");
                    }
                }
            }
            out.push_str("}\n\n");
        }

        out.push_str("\n!0 = !{!0, !1}\n!1 = !{!\"llvm.loop.vectorize.enable\", i1 true}\n");
        Ok(out)
    }
}

fn llvm_global_ident(name: &str) -> String {
    // LLVM global identifiers can be quoted: @"a::b".
    // Quote anything that's not a simple C-like identifier or starts with a digit.
    fn is_simple(s: &str) -> bool {
        let mut chars = s.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        if first.is_ascii_digit() {
            return false;
        }
        if !(first.is_ascii_alphabetic() || matches!(first, '_' | '.' | '$')) {
            return false;
        }
        chars.all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '.' | '$'))
    }

    if is_simple(name) {
        format!("@{name}")
    } else {
        let mut escaped = String::with_capacity(name.len());
        for ch in name.chars() {
            match ch {
                '\\' => escaped.push_str("\\\\"),
                '"' => escaped.push_str("\\\""),
                _ => escaped.push(ch),
            }
        }
        format!("@\"{escaped}\"")
    }
}

fn llvm_block_ident(name: &str, ordinal: usize) -> String {
    let mut sanitized = String::with_capacity(name.len() + 8);
    sanitized.push_str("bb_");
    for ch in name.chars() {
        if ch.is_ascii_alphanumeric() || matches!(ch, '_' | '$') {
            sanitized.push(ch);
        } else {
            sanitized.push('_');
        }
    }
    sanitized.push('_');
    sanitized.push_str(&ordinal.to_string());
    sanitized
}

fn label_name<'a>(label: &'a str, labels: &'a HashMap<String, String>) -> &'a str {
    labels.get(label).map(String::as_str).unwrap_or(label)
}

fn value_to_llvm(
    value: &Value,
    locals: &HashMap<String, TypedValue>,
    strings: &StringTable,
) -> Result<TypedValue, String> {
    match value {
        Value::Int(v) => Ok(TypedValue {
            repr: v.to_string(),
            ty: LlvmType::I64,
        }),
        Value::Float(v) => Ok(TypedValue {
            repr: format!("{v}"),
            ty: LlvmType::F64,
        }),
        Value::Bool(b) => Ok(TypedValue {
            repr: if *b { "1" } else { "0" }.to_string(),
            ty: LlvmType::I64,
        }),
        Value::Null => Ok(TypedValue {
            repr: "0".to_string(),
            ty: LlvmType::I64,
        }),
        Value::Str(s) => Ok(TypedValue {
            repr: strings.lookup(s)?,
            ty: LlvmType::Ptr,
        }),
        Value::Local(name) | Value::Temp(name) => locals
            .get(name)
            .cloned()
            .ok_or_else(|| format!("undefined local '{name}' during LLVM lowering")),
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
enum LlvmType {
    I64,
    F64,
    F32,
    Ptr,
    Struct(String),
    Vector(Box<LlvmType>, usize),
}

fn llvm_type(ty: &LlvmType) -> String {
    match ty {
        LlvmType::I64 => "i64".to_string(),
        LlvmType::F64 => "double".to_string(),
        LlvmType::F32 => "float".to_string(),
        LlvmType::Ptr => "ptr".to_string(),
        LlvmType::Struct(_) => "ptr".to_string(),
        LlvmType::Vector(inner, len) => format!("<{} x {}>", len, llvm_type(inner)),
    }
}

fn llvm_type_string(ty: &LlvmType) -> String {
    match ty {
        LlvmType::I64 => "i64".to_string(),
        LlvmType::F64 => "double".to_string(),
        LlvmType::F32 => "float".to_string(),
        LlvmType::Ptr => "ptr".to_string(),
        LlvmType::Struct(name) => format!("%{name}"),
        LlvmType::Vector(inner, len) => format!("<{} x {}>", len, llvm_type(inner)),
    }
}

fn llvm_type_from_ir(ty: &IrType) -> String {
    match ty {
        IrType::I64 => "i64".to_string(),
        IrType::F64 => "double".to_string(),
        IrType::Ptr => "ptr".to_string(),
        IrType::Struct(name) => format!("%{name}"),
        IrType::Array(inner) => {
            let elem = llvm_type_from_ir(inner);
            format!("%{}", array_struct_name(&elem))
        }
    }
}

fn llvm_type_to_llvmt(ty: String) -> Result<LlvmType, String> {
    match ty.as_str() {
        "i64" => Ok(LlvmType::I64),
        "double" => Ok(LlvmType::F64),
        "ptr" => Ok(LlvmType::Ptr),
        _ if ty.starts_with('%') => Ok(LlvmType::Struct(ty.trim_start_matches('%').to_string())),
        _ => Err(format!("unknown LLVM type '{ty}'")),
    }
}

fn build_struct_map(structs: &[StructDef]) -> HashMap<String, StructDef> {
    let mut out = HashMap::new();
    for s in structs {
        out.insert(s.name.clone(), s.clone());
    }
    out
}

fn array_struct_name(elem_ty: &str) -> String {
    let mut name = elem_ty
        .trim_start_matches('%')
        .replace('*', "ptr")
        .replace("::", "_")
        .replace('.', "_");
    if name.is_empty() {
        name = "anon".to_string();
    }
    format!("nyx_array_{}", name)
}

fn collect_array_types(module: &Module) -> Vec<ArrayDef> {
    let mut out: HashMap<String, ArrayDef> = HashMap::new();
    for func in &module.functions {
        for instr in &func.instructions {
            match instr {
                Instruction::ArrayInit { elem_ty, .. }
                | Instruction::ArrayGet { elem_ty, .. }
                | Instruction::ArraySet { elem_ty, .. } => {
                    let elem = llvm_type_from_ir(elem_ty);
                    let name = array_struct_name(&elem);
                    out.entry(name.clone()).or_insert(ArrayDef { name, elem_ty: elem });
                }
                _ => {}
            }
        }
    }
    out.into_values().collect()
}

#[derive(Clone)]
struct ArrayDef {
    name: String,
    elem_ty: String,
}

#[derive(Debug, Clone)]
struct TypedValue {
    repr: String,
    ty: LlvmType,
}

fn emit_compare(
    op: &BinaryOp,
    lhs: &TypedValue,
    rhs: &TypedValue,
    ids: &mut usize,
) -> Result<(String, String), String> {
    let cmp_reg = format!("%v{ids}");
    *ids += 1;
    let mut line = String::new();
    match (&lhs.ty, &rhs.ty) {
        (LlvmType::I64, LlvmType::I64) => {
            let pred = match op {
                BinaryOp::Eq => "eq",
                BinaryOp::Ne => "ne",
                BinaryOp::Lt => "slt",
                BinaryOp::Le => "sle",
                BinaryOp::Gt => "sgt",
                BinaryOp::Ge => "sge",
                _ => return Err("invalid integer comparison op".into()),
            };
            write!(
                &mut line,
                "  {cmp_reg} = icmp {pred} i64 {}, {}\n",
                lhs.repr, rhs.repr
            )
            .map_err(|e| e.to_string())?;
        }
        (LlvmType::F64, LlvmType::F64) => {
            let pred = match op {
                BinaryOp::Eq => "oeq",
                BinaryOp::Ne => "one",
                BinaryOp::Lt => "olt",
                BinaryOp::Le => "ole",
                BinaryOp::Gt => "ogt",
                BinaryOp::Ge => "oge",
                _ => return Err("invalid float comparison op".into()),
            };
            write!(
                &mut line,
                "  {cmp_reg} = fcmp {pred} double {}, {}\n",
                lhs.repr, rhs.repr
            )
            .map_err(|e| e.to_string())?;
        }
        _ => return Err("comparison operands must have matching types".into()),
    }
    Ok((cmp_reg, line))
}

fn emit_binary(
    op: &BinaryOp,
    lhs: &TypedValue,
    rhs: &TypedValue,
    dst: &str,
) -> Result<(String, LlvmType), String> {
    match (&lhs.ty, &rhs.ty) {
        (LlvmType::I64, LlvmType::I64) => {
            let llvm_op = match op {
                BinaryOp::Add => "add",
                BinaryOp::Sub => "sub",
                BinaryOp::Mul => "mul",
                BinaryOp::Div => "sdiv",
                BinaryOp::Mod => "srem",
                BinaryOp::Shl => "shl",
                BinaryOp::Shr => "ashr",
                BinaryOp::BitAnd => "and",
                BinaryOp::BitOr => "or",
                BinaryOp::BitXor => "xor",
                _ => return Err(format!("unsupported integer op: {op:?}")),
            };
            Ok((
                format!("  {dst} = {llvm_op} i64 {}, {}\n", lhs.repr, rhs.repr),
                LlvmType::I64,
            ))
        }
        (LlvmType::F64, LlvmType::F64) => {
            let llvm_op = match op {
                BinaryOp::Add => "fadd",
                BinaryOp::Sub => "fsub",
                BinaryOp::Mul => "fmul",
                BinaryOp::Div => "fdiv",
                _ => return Err(format!("unsupported float op: {op:?}")),
            };
            Ok((
                format!(
                    "  {dst} = {llvm_op} double {}, {}\n",
                    lhs.repr, rhs.repr
                ),
                LlvmType::F64,
            ))
        }
        _ => Err("binary operands must have matching numeric types".into()),
    }
}

#[derive(Default)]
struct StringTable {
    map: HashMap<String, String>,
    defs: Vec<String>,
}

impl StringTable {
    fn intern(&mut self, s: &str) -> String {
        if let Some(name) = self.map.get(s) {
            return name.clone();
        }
        let id = self.map.len();
        let symbol = format!("@.str.{id}");
        let escaped = escape_llvm_string(s);
        let len = s.as_bytes().len() + 1;
        let def = format!(
            "{symbol} = private unnamed_addr constant [{len} x i8] c\"{escaped}\\00\"\n"
        );
        self.map.insert(s.to_string(), symbol.clone());
        self.defs.push(def);
        symbol
    }

    fn lookup(&self, s: &str) -> Result<String, String> {
        self.map
            .get(s)
            .cloned()
            .ok_or_else(|| "string literal missing from table".to_string())
    }

    fn definitions(&self) -> Vec<String> {
        self.defs.clone()
    }
}

fn escape_llvm_string(s: &str) -> String {
    let mut out = String::new();
    for ch in s.bytes() {
        match ch {
            b'\\' => out.push_str("\\\\"),
            b'"' => out.push_str("\\22"),
            b'\n' => out.push_str("\\0A"),
            b'\r' => out.push_str("\\0D"),
            b'\t' => out.push_str("\\09"),
            0x20..=0x7e => out.push(ch as char),
            _ => {
                let _ = write!(&mut out, "\\{:02X}", ch);
            }
        }
    }
    out
}

fn collect_string_literals(module: &Module, strings: &mut StringTable) {
    for func in &module.functions {
        for instr in &func.instructions {
            match instr {
                Instruction::Let { value, .. }
                | Instruction::Print { value }
                | Instruction::Return { value: Some(value) } => {
                    if let Value::Str(s) = value {
                        strings.intern(s);
                    }
                }
                Instruction::Binary { lhs, rhs, .. } => {
                    if let Value::Str(s) = lhs {
                        strings.intern(s);
                    }
                    if let Value::Str(s) = rhs {
                        strings.intern(s);
                    }
                }
                Instruction::Call { args, .. } => {
                    for arg in args {
                        if let Value::Str(s) = arg {
                            strings.intern(s);
                        }
                    }
                }
                Instruction::StructInit { fields, .. } => {
                    for (_, v) in fields {
                        if let Value::Str(s) = v {
                            strings.intern(s);
                        }
                    }
                }
                Instruction::ArrayInit { len, .. } => {
                    if let Value::Str(s) = len {
                        strings.intern(s);
                    }
                }
                Instruction::ArraySet { value, index, .. } => {
                    if let Value::Str(s) = value {
                        strings.intern(s);
                    }
                    if let Value::Str(s) = index {
                        strings.intern(s);
                    }
                }
                Instruction::ArrayGet { index, .. } => {
                    if let Value::Str(s) = index {
                        strings.intern(s);
                    }
                }
                Instruction::InlineAsm { inputs, .. } => {
                    for arg in inputs {
                        if let Value::Str(s) = &arg.value {
                            strings.intern(s);
                        }
                    }
                }
                Instruction::Branch { cond, .. } => {
                    if let Value::Str(s) = cond {
                        strings.intern(s);
                    }
                }
                _ => {}
            }
        }
    }
}
