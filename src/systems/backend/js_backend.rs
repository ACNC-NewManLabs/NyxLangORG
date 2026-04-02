use crate::systems::ir::nyx_ir::{BinaryOp, Instruction, Module, Value};

#[derive(Debug, Default)]
pub struct JsBackend;

impl JsBackend {
    pub fn lower_to_js(&self, module: &Module) -> Result<String, String> {
        let mut js = String::new();
        // A minimal shim to provide DOM/Browser compatibility from Nyx
        js.push_str("function print(val) { console.log(val); }\n");
        js.push_str("function js_eval(code) { return eval(code); }\n\n");
        js.push_str("function nyx_inline_asm(code, inputs) {\n");
        js.push_str("  const fn = new Function('inputs', code);\n");
        js.push_str("  return fn(inputs);\n");
        js.push_str("}\n\n");

        for func in &module.functions {
            // Include `_args` simply to capture anything passed
            let mut params = Vec::new();
            for idx in 0..func.params.len() {
                params.push(format!("arg{idx}"));
            }
            js.push_str(&format!(
                "function {}({}) {{\n",
                func.name,
                params.join(", ")
            ));
            js.push_str("  let __state = 0;\n");
            js.push_str("  while (true) {\n");
            js.push_str("    switch (__state) {\n");
            js.push_str("      case 0:\n");
            for (idx, p) in func.params.iter().enumerate() {
                let safe_name = p.name.replace("::", "_");
                js.push_str(&format!("        var {} = arg{};\n", safe_name, idx));
            }

            let mut state_id = 0;
            let mut label_to_state = std::collections::HashMap::new();

            for instr in &func.instructions {
                if let Instruction::Label(lbl) = instr {
                    state_id += 1;
                    label_to_state.insert(lbl.clone(), state_id);
                }
            }

            for instr in &func.instructions {
                match instr {
                    Instruction::Let { name, value } => {
                        let val = self.val_to_js(value);
                        let safe_name = name.replace("::", "_");
                        js.push_str(&format!("        var {} = {};\n", safe_name, val));
                    }
                    Instruction::Print { value } => {
                        let val = self.val_to_js(value);
                        js.push_str(&format!("        print({});\n", val));
                    }
                    Instruction::Return { value } => {
                        if let Some(v) = value {
                            let val = self.val_to_js(v);
                            js.push_str(&format!("        return {};\n", val));
                        } else {
                            js.push_str("        return;\n");
                        }
                    }
                    Instruction::Binary { dst, op, lhs, rhs } => {
                        let l = self.val_to_js(lhs);
                        let r = self.val_to_js(rhs);
                        let safe_dst = dst.replace("::", "_");

                        if matches!(op, BinaryOp::Not) {
                            js.push_str(&format!("        var {} = !({});\n", safe_dst, r));
                        } else {
                            let op_str = match op {
                                BinaryOp::Add => "+",
                                BinaryOp::Sub => "-",
                                BinaryOp::Mul => "*",
                                BinaryOp::Div => "/",
                                BinaryOp::Mod => "%",
                                BinaryOp::Eq => "===",
                                BinaryOp::Ne => "!==",
                                BinaryOp::Lt => "<",
                                BinaryOp::Le => "<=",
                                BinaryOp::Gt => ">",
                                BinaryOp::Ge => ">=",
                                BinaryOp::And => "&&",
                                BinaryOp::Or => "||",
                                BinaryOp::BitAnd => "&",
                                BinaryOp::BitOr => "|",
                                BinaryOp::BitXor => "^",
                                BinaryOp::Shl => "<<",
                                BinaryOp::Shr => ">>",
                                BinaryOp::Not => "!",
                            };
                            js.push_str(&format!(
                                "        var {} = {} {} {};\n",
                                safe_dst, l, op_str, r
                            ));
                        }
                    }
                    Instruction::Call { dst, callee, args } => {
                        let arg_strs: Vec<String> =
                            args.iter().map(|a| self.val_to_js(a)).collect();
                        let safe_dst = dst.replace("::", "_");

                        let safe_callee = callee.replace("::", "_");

                        js.push_str(&format!(
                            "        var {} = {}({});\n",
                            safe_dst,
                            safe_callee,
                            arg_strs.join(", ")
                        ));
                    }
                    Instruction::StructInit {
                        dst,
                        struct_name: _,
                        fields,
                    } => {
                        let safe_dst = dst.replace("::", "_");
                        let mut parts = Vec::new();
                        for (name, val) in fields {
                            let v = self.val_to_js(val);
                            if Self::js_ident(name) {
                                parts.push(format!("{}: {}", name, v));
                            } else {
                                parts.push(format!("{}: {}", Self::js_str_lit(name), v));
                            }
                        }
                        js.push_str(&format!(
                            "        var {} = {{ {} }};\n",
                            safe_dst,
                            parts.join(", ")
                        ));
                    }
                    Instruction::StructGet {
                        dst, base, field, ..
                    } => {
                        let safe_dst = dst.replace("::", "_");
                        let safe_base = base.replace("::", "_");
                        let access = self.js_prop_access(&safe_base, field);
                        js.push_str(&format!("        var {} = {};\n", safe_dst, access));
                    }
                    Instruction::ArrayInit { dst, len, .. } => {
                        let safe_dst = dst.replace("::", "_");
                        let len_val = self.val_to_js(len);
                        js.push_str(&format!(
                            "        var {} = new Array({});\n",
                            safe_dst, len_val
                        ));
                    }
                    Instruction::ArraySet {
                        base, index, value, ..
                    } => {
                        let safe_base = base.replace("::", "_");
                        let idx = self.val_to_js(index);
                        let val = self.val_to_js(value);
                        js.push_str(&format!("        {}[{}] = {};\n", safe_base, idx, val));
                    }
                    Instruction::ArrayGet {
                        dst, base, index, ..
                    } => {
                        let safe_dst = dst.replace("::", "_");
                        let safe_base = base.replace("::", "_");
                        let idx = self.val_to_js(index);
                        js.push_str(&format!(
                            "        var {} = {}[{}];\n",
                            safe_dst, safe_base, idx
                        ));
                    }
                    Instruction::InlineAsm {
                        code,
                        outputs,
                        inputs,
                    } => {
                        let input_vals: Vec<String> =
                            inputs.iter().map(|i| self.val_to_js(&i.value)).collect();
                        let inputs_js = format!("[{}]", input_vals.join(", "));
                        let code_js = Self::js_str_lit(code);
                        if outputs.is_empty() {
                            js.push_str(&format!(
                                "        nyx_inline_asm({}, {});\n",
                                code_js, inputs_js
                            ));
                        } else if outputs.len() == 1 {
                            let name = outputs[0].name.replace("::", "_");
                            js.push_str(&format!(
                                "        var {} = nyx_inline_asm({}, {});\n",
                                name, code_js, inputs_js
                            ));
                        } else {
                            js.push_str(&format!(
                                "        var __asm_out = nyx_inline_asm({}, {});\n",
                                code_js, inputs_js
                            ));
                            for (idx, outp) in outputs.iter().enumerate() {
                                let name = outp.name.replace("::", "_");
                                js.push_str(&format!(
                                    "        var {} = __asm_out[{}];\n",
                                    name, idx
                                ));
                            }
                        }
                    }
                    Instruction::Label(lbl) => {
                        let id = label_to_state.get(lbl).unwrap();
                        js.push_str(&format!("        __state = {}; break;\n", id));
                        js.push_str(&format!("      case {}:\n", id));
                    }
                    Instruction::Jump(lbl) => {
                        let id = label_to_state.get(lbl).unwrap();
                        js.push_str(&format!("        __state = {}; break;\n", id));
                    }
                    Instruction::Branch {
                        cond,
                        then_label,
                        else_label,
                    } => {
                        let cond_str = self.val_to_js(cond);
                        let then_id = label_to_state.get(then_label).unwrap();
                        let else_id = label_to_state.get(else_label).unwrap();
                        js.push_str(&format!("        if ({}) {{ __state = {}; break; }} else {{ __state = {}; break; }}\n", cond_str, then_id, else_id));
                    }
                }
            }
            js.push_str("    }\n  }\n}\n\n");
            // Export the function out to the global window
            js.push_str(&format!(
                "if (typeof window !== 'undefined') window.{} = {};\n",
                func.name, func.name
            ));
        }

        // Auto-execution for 'main'
        js.push_str("\nif (typeof window !== 'undefined') {\n");
        js.push_str("  window.addEventListener('load', () => {\n");
        js.push_str("    if (typeof main === 'function') {\n");
        js.push_str("      main();\n");
        js.push_str("    }\n");
        js.push_str("  });\n");
        js.push_str("}\n");

        Ok(js)
    }

    fn val_to_js(&self, val: &Value) -> String {
        match val {
            Value::Int(i) => i.to_string(),
            Value::Float(f) => format!("{f}"),
            Value::Bool(b) => if *b { "true" } else { "false" }.to_string(),
            Value::Str(s) => format!("`{}`", s.replace("`", "\\`")),
            Value::Null => "null".to_string(),
            Value::Local(name) | Value::Temp(name) => name.replace("::", "_"),
        }
    }

    fn js_ident(name: &str) -> bool {
        let mut chars = name.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        if !(first.is_ascii_alphabetic() || first == '_' || first == '$') {
            return false;
        }
        chars.all(|c| c.is_ascii_alphanumeric() || c == '_' || c == '$')
    }

    fn js_str_lit(s: &str) -> String {
        format!("{:?}", s)
    }

    fn js_prop_access(&self, base: &str, field: &str) -> String {
        if Self::js_ident(field) {
            format!("{base}.{field}")
        } else {
            format!("{base}[{}]", Self::js_str_lit(field))
        }
    }
}
