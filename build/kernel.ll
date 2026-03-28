target triple = "x86_64-unknown-none-elf"
@multiboot_header = dso_local constant { i32, i32, i32, i32 } { i32 -397250346, i32 0, i32 16, i32 397250330 }, section ".multiboot", align 8
@.fmt_i64 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.fmt_f64 = private unnamed_addr constant [4 x i8] c"%f\0A\00"
@.fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00"
@.str.0 = private unnamed_addr constant [39 x i8] c"======================================\00"
@.str.1 = private unnamed_addr constant [39 x i8] c"        Nyx Bare-Metal OS Kernel      \00"
@.str.2 = private unnamed_addr constant [32 x i8] c"[1/4] PMM (Physical Memory)... \00"
@.str.3 = private unnamed_addr constant [3 x i8] c"OK\00"
@.str.4 = private unnamed_addr constant [35 x i8] c"[2/4] Malloc (Heap Allocation)... \00"
@.str.5 = private unnamed_addr constant [31 x i8] c"[3/4] Interrupts (IDT/PIC)... \00"
@.str.6 = private unnamed_addr constant [34 x i8] c"[4/4] Shell Environment... READY.\00"
@.str.7 = private unnamed_addr constant [46 x i8] c"\0AAvailable Commands: help, mem, clear, reboot\00"
@.str.8 = private unnamed_addr constant [11 x i8] c"\0Anyx_os:> \00"
@.str.9 = private unnamed_addr constant [10 x i8] c"nyx_os:> \00"
@.str.10 = private unnamed_addr constant [5 x i8] c"help\00"
@.str.11 = private unnamed_addr constant [11 x i8] c"Help Menu:\00"
@.str.12 = private unnamed_addr constant [37 x i8] c"  help   - Display this command list\00"
@.str.13 = private unnamed_addr constant [49 x i8] c"  mem    - Print kernel heap and RAM information\00"
@.str.14 = private unnamed_addr constant [37 x i8] c"  clear  - Flush the VGA text buffer\00"
@.str.15 = private unnamed_addr constant [42 x i8] c"  reboot - Trigger a warm reboot via PS/2\00"
@.str.16 = private unnamed_addr constant [4 x i8] c"mem\00"
@.str.17 = private unnamed_addr constant [28 x i8] c"--- Physical Memory Map ---\00"
@.str.18 = private unnamed_addr constant [37 x i8] c"  Lower Memory: 0x000000 -> 0x0A0000\00"
@.str.19 = private unnamed_addr constant [25 x i8] c"  VGA Buffer  : 0x0B8000\00"
@.str.20 = private unnamed_addr constant [25 x i8] c"  Kernel Start: 0x100000\00"
@.str.21 = private unnamed_addr constant [42 x i8] c"  Kernel Heap : 0x1000000 (256KB Dynamic)\00"
@.str.22 = private unnamed_addr constant [6 x i8] c"clear\00"
@.str.23 = private unnamed_addr constant [7 x i8] c"reboot\00"
@.str.24 = private unnamed_addr constant [13 x i8] c"Rebooting...\00"
@.str.25 = private unnamed_addr constant [9 x i8] c"Error: '\00"
@.str.26 = private unnamed_addr constant [38 x i8] c"' is not a recognized system command.\00"

declare i64 @char_at(...)
declare i64 @"interrupts::enable"(...)
declare i64 @"interrupts::init"(...)
declare i64 @"keyboard::get_char"(...)
declare i64 @len(...)
declare i64 @"malloc::init"(...)
declare i64 @"malloc::malloc"(...)
declare i64 @"pmm::init"(...)
declare i64 @"std::kernel::memory::read_byte"(...)
declare i64 @"std::kernel::memory::write_byte"(...)
declare i64 @"std::kernel::ports::outb"(...)
declare i64 @"vga::clear"(...)
declare i64 @"vga::get_cursor_x"(...)
declare i64 @"vga::print_char"(...)
declare i64 @"vga::print_str"(...)
declare i64 @"vga::println"(...)
declare i64 @"vga::set_cursor_x"(...)
declare i64 @"vga::update_cursor"(...)
define i32 @main() {
entry:
  %v0 = call i64 @"vga::clear"()
  %v1 = call i64 @"vga::println"(ptr @.str.0)
  %v2 = call i64 @"vga::println"(ptr @.str.1)
  %v3 = call i64 @"vga::println"(ptr @.str.0)
  %v4 = call i64 @"vga::print_str"(ptr @.str.2)
  %v5 = call i64 @"pmm::init"()
  %v6 = call i64 @"vga::println"(ptr @.str.3)
  %v7 = call i64 @"vga::print_str"(ptr @.str.4)
  %v8 = call i64 @"malloc::init"(i64 64)
  %v9 = call i64 @"vga::println"(ptr @.str.3)
  %v10 = call i64 @"vga::print_str"(ptr @.str.5)
  %v11 = call i64 @"interrupts::init"()
  %v12 = call i64 @"vga::println"(ptr @.str.3)
  %v13 = call i64 @"vga::println"(ptr @.str.6)
  %v14 = call i64 @"vga::println"(ptr @.str.7)
  %v15 = call i64 @"vga::print_str"(ptr @.str.8)
  %v16 = call i64 @"malloc::malloc"(i64 128)
  %v17 = add i64 0, %v16
  %v18 = add i64 0, 0
  %v19 = call i64 @"interrupts::enable"()
  br label %bb_while_cond_0_0
bb_while_cond_0_0:
  %v20 = icmp ne i64 1, 0
  br i1 %v20, label %bb_while_body_1_1, label %bb_while_end_2_12
bb_while_body_1_1:
  %v21 = call i64 @"keyboard::get_char"()
  %v22 = add i64 0, %v21
  %v23 = icmp ne i64 %v22, 0
  %v24 = zext i1 %v23 to i64
  %v25 = icmp ne i64 %v24, 0
  br i1 %v25, label %bb_then_4_2, label %bb_if_end_3_11
bb_then_4_2:
  %v26 = icmp eq i64 %v22, 10
  %v27 = zext i1 %v26 to i64
  %v28 = icmp ne i64 %v27, 0
  br i1 %v28, label %bb_then_7_3, label %bb_else_6_7
bb_then_7_3:
  %v29 = call i64 @"vga::print_char"(i64 10)
  %v30 = call i64 @"kernel_shell::handle_command"(i64 %v17, i64 %v18)
  %v31 = add i64 0, 0
  %v32 = call i64 @"vga::print_str"(ptr @.str.9)
  br label %bb_if_end_5_10
  %v33 = icmp eq i64 %v22, 8
  %v34 = zext i1 %v33 to i64
  %v35 = icmp ne i64 %v34, 0
  br i1 %v35, label %bb_then_8_4, label %bb_else_6_7
bb_then_8_4:
  %v36 = icmp sgt i64 %v31, 0
  %v37 = zext i1 %v36 to i64
  %v38 = icmp ne i64 %v37, 0
  br i1 %v38, label %bb_then_10_5, label %bb_if_end_9_6
bb_then_10_5:
  %v39 = sub i64 %v31, 1
  %v40 = add i64 0, %v39
  %v41 = call i64 @"vga::get_cursor_x"()
  %v42 = sub i64 %v41, 1
  %v43 = add i64 0, %v42
  %v44 = call i64 @"vga::set_cursor_x"(i64 %v43)
  %v45 = call i64 @"vga::print_char"(i64 32)
  %v46 = call i64 @"vga::set_cursor_x"(i64 %v43)
  %v47 = call i64 @"vga::update_cursor"()
  br label %bb_if_end_9_6
bb_if_end_9_6:
  br label %bb_if_end_5_10
bb_else_6_7:
  %v48 = icmp slt i64 %v40, 127
  %v49 = zext i1 %v48 to i64
  %v50 = icmp ne i64 %v49, 0
  br i1 %v50, label %bb_then_12_8, label %bb_if_end_11_9
bb_then_12_8:
  %v51 = add i64 %v17, %v40
  %v52 = inttoptr i64 %v51 to i8*
  %v53 = trunc i64 %v22 to i8
  store volatile i8 %v53, i8* %v52
  %v54 = add i64 %v40, 1
  %v55 = add i64 0, %v54
  %v56 = call i64 @"vga::print_char"(i64 %v22)
  br label %bb_if_end_11_9
bb_if_end_11_9:
  br label %bb_if_end_5_10
bb_if_end_5_10:
  br label %bb_if_end_3_11
bb_if_end_3_11:
  br label %bb_while_cond_0_0
bb_while_end_2_12:
  ret i32 0
}

define i64 @"kernel_shell::handle_command"(i64 %arg0, i64 %arg1) {
entry:
  %v0 = icmp sle i64 %arg1, 0
  %v1 = zext i1 %v0 to i64
  %v2 = icmp ne i64 %v1, 0
  br i1 %v2, label %bb_then_14_0, label %bb_if_end_13_1
bb_then_14_0:
  ret i64 0
  br label %bb_if_end_13_1
bb_if_end_13_1:
  %v3 = call i64 @"kernel_shell::streq"(i64 %arg0, i64 %arg1, ptr @.str.10)
  %v4 = icmp ne i64 %v3, 0
  br i1 %v4, label %bb_then_17_2, label %bb_else_16_6
bb_then_17_2:
  %v5 = call i64 @"vga::println"(ptr @.str.11)
  %v6 = call i64 @"vga::println"(ptr @.str.12)
  %v7 = call i64 @"vga::println"(ptr @.str.13)
  %v8 = call i64 @"vga::println"(ptr @.str.14)
  %v9 = call i64 @"vga::println"(ptr @.str.15)
  br label %bb_if_end_15_10
  %v10 = call i64 @"kernel_shell::streq"(i64 %arg0, i64 %arg1, ptr @.str.16)
  %v11 = icmp ne i64 %v10, 0
  br i1 %v11, label %bb_then_18_3, label %bb_else_16_6
bb_then_18_3:
  %v12 = call i64 @"vga::println"(ptr @.str.17)
  %v13 = call i64 @"vga::println"(ptr @.str.18)
  %v14 = call i64 @"vga::println"(ptr @.str.19)
  %v15 = call i64 @"vga::println"(ptr @.str.20)
  %v16 = call i64 @"vga::println"(ptr @.str.21)
  br label %bb_if_end_15_10
  %v17 = call i64 @"kernel_shell::streq"(i64 %arg0, i64 %arg1, ptr @.str.22)
  %v18 = icmp ne i64 %v17, 0
  br i1 %v18, label %bb_then_19_4, label %bb_else_16_6
bb_then_19_4:
  %v19 = call i64 @"vga::clear"()
  br label %bb_if_end_15_10
  %v20 = call i64 @"kernel_shell::streq"(i64 %arg0, i64 %arg1, ptr @.str.23)
  %v21 = icmp ne i64 %v20, 0
  br i1 %v21, label %bb_then_20_5, label %bb_else_16_6
bb_then_20_5:
  %v22 = call i64 @"vga::println"(ptr @.str.24)
  %v23 = trunc i64 100 to i16
  %v24 = trunc i64 254 to i8
  call void asm sideeffect "outb $0, $1", "{al},{dx},~{dirflag},~{fpsr},~{flags}"(i8 %v24, i16 %v23)
  br label %bb_if_end_15_10
bb_else_16_6:
  %v25 = call i64 @"vga::print_str"(ptr @.str.25)
  %v26 = add i64 0, 0
  br label %bb_while_cond_21_7
bb_while_cond_21_7:
  %v27 = icmp slt i64 %v26, %arg1
  %v28 = zext i1 %v27 to i64
  %v29 = icmp ne i64 %v28, 0
  br i1 %v29, label %bb_while_body_22_8, label %bb_while_end_23_9
bb_while_body_22_8:
  %v30 = add i64 %arg0, %v26
  %v31 = inttoptr i64 %v30 to i8*
  %v32 = load volatile i8, i8* %v31
  %v33 = zext i8 %v32 to i64
  %v34 = call i64 @"vga::print_char"(i64 %v33)
  %v35 = add i64 %v26, 1
  %v36 = add i64 0, %v35
  br label %bb_while_cond_21_7
bb_while_end_23_9:
  %v37 = call i64 @"vga::println"(ptr @.str.26)
  br label %bb_if_end_15_10
bb_if_end_15_10:
  ret i64 0
}

define i64 @"kernel_shell::streq"(i64 %arg0, i64 %arg1, i64 %arg2) {
entry:
  %v1 = inttoptr i64 %arg2 to ptr
  br label %len_loop_2
len_loop_2:
  %phi_2 = phi i64 [ 0, %entry ], [ %next_2, %len_loop_2 ]
  %ch_2 = getelementptr i8, ptr %v1, i64 %phi_2
  %lch_3 = load i8, ptr %ch_2
  %cond_2 = icmp ne i8 %lch_3, 0
  %next_2 = add i64 %phi_2, 1
  br i1 %cond_2, label %len_loop_2, label %len_end_2
len_end_2:
  %v0 = add i64 %phi_2, 0
  %v4 = add i64 0, %v0
  %v5 = icmp ne i64 %arg1, %v4
  %v6 = zext i1 %v5 to i64
  %v7 = icmp ne i64 %v6, 0
  br i1 %v7, label %bb_then_25_0, label %bb_if_end_24_1
bb_then_25_0:
  ret i64 0
  br label %bb_if_end_24_1
bb_if_end_24_1:
  %v8 = add i64 0, 0
  br label %bb_while_cond_26_2
bb_while_cond_26_2:
  %v9 = icmp slt i64 %v8, %arg1
  %v10 = zext i1 %v9 to i64
  %v11 = icmp ne i64 %v10, 0
  br i1 %v11, label %bb_while_body_27_3, label %bb_while_end_28_6
bb_while_body_27_3:
  %v12 = add i64 %arg0, %v8
  %v13 = inttoptr i64 %v12 to i8*
  %v14 = load volatile i8, i8* %v13
  %v15 = zext i8 %v14 to i64
  %v16 = add i64 0, %v15
  %v17 = inttoptr i64 %arg2 to ptr
  %v19 = getelementptr i8, ptr %v17, i64 %v8
  %v18 = load i8, ptr %v19
  %v20 = zext i8 %v18 to i64
  %v21 = add i64 0, %v20
  %v22 = icmp ne i64 %v16, %v21
  %v23 = zext i1 %v22 to i64
  %v24 = icmp ne i64 %v23, 0
  br i1 %v24, label %bb_then_30_4, label %bb_if_end_29_5
bb_then_30_4:
  ret i64 0
  br label %bb_if_end_29_5
bb_if_end_29_5:
  %v25 = add i64 %v8, 1
  %v26 = add i64 0, %v25
  br label %bb_while_cond_26_2
bb_while_end_28_6:
  ret i64 1
}

