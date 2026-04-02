target triple = "x86_64-unknown-none-elf"
@multiboot_header = dso_local constant { i32, i32, i32, i32 } { i32 -397250346, i32 0, i32 16, i32 397250330 }, section ".multiboot", align 8
@.fmt_i64 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.fmt_f64 = private unnamed_addr constant [4 x i8] c"%f\0A\00"
@.fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00"
@.str.0 = private unnamed_addr constant [27 x i8] c"[  OK  ] Starting klogd...\00"
@.str.1 = private unnamed_addr constant [37 x i8] c"[  OK  ] Bringing up interface lo...\00"
@.str.2 = private unnamed_addr constant [29 x i8] c"[  OK  ] Mounting /sys /proc\00"
@.str.3 = private unnamed_addr constant [23 x i8] c"[  OK  ] Starting ntpd\00"
@.str.4 = private unnamed_addr constant [29 x i8] c"Welcome to Alpine Linux 3.19\00"
@.str.5 = private unnamed_addr constant [31 x i8] c"Kernel 6.6.14-0-virt on x86_64\00"
@.str.6 = private unnamed_addr constant [18 x i8] c"localhost login: \00"

declare i64 @"std::kernel::cpu::hlt"(...)
declare i64 @"std::kernel::memory::read_u32"(...)
declare i64 @"std::kernel::memory::write_u32"(...)
declare i64 @"std::kernel::vm::get_fb_ptr"(...)
declare i64 @"std::kernel::vm::get_input_ptr"(...)
define i32 @main() {
entry:
  %v0 = call i64 asm sideeffect "mov $$100, %rax; .byte 0xf1", "={rax}"()
  %v1 = add i64 0, %v0
  %v2 = call i64 asm sideeffect "mov $$101, %rax; .byte 0xf1", "={rax}"()
  %v3 = add i64 0, %v2
  %v4 = add i64 0, 800
  %v5 = add i64 0, 600
  %v6 = add i64 0, 4279374354
  %v7 = add i64 0, 4292927712
  %v8 = add i64 0, 4278252287
  %v9 = add i64 0, 4287137928
  %v10 = add i64 0, 0
  %v11 = add i64 0, 0
  %v12 = add i64 0, 0
  %v13 = add i64 0, 0
  %v14 = add i64 0, 0
  %v15 = add i64 0, 0
  %v16 = add i64 0, 0
  br label %bb_loop_0_0, !llvm.loop !0
bb_loop_0_0:
  %v17 = call i64 @"alpine::draw_rect"(i64 %v1, i64 0, i64 0, i64 %v4, i64 %v5, i64 %v6)
  %v18 = call i64 @"alpine::draw_rect"(i64 %v1, i64 20, i64 20, i64 760, i64 2, i64 %v8)
  %v19 = call i64 @"alpine::draw_alpine_logo"(i64 %v1, i64 600, i64 50)
  %v20 = add i64 %v10, 1
  %v21 = add i64 0, %v20
  %v22 = icmp sgt i64 %v21, 10
  %v23 = zext i1 %v22 to i64
  %v24 = icmp ne i64 %v23, 0
  br i1 %v24, label %bb_then_3_1, label %bb_if_end_2_2
bb_then_3_1:
  %v25 = call i64 @"alpine::draw_log"(i64 %v1, i64 40, i64 60, ptr @.str.0, i64 %v9)
  br label %bb_if_end_2_2
bb_if_end_2_2:
  %v26 = icmp sgt i64 %v21, 30
  %v27 = zext i1 %v26 to i64
  %v28 = icmp ne i64 %v27, 0
  br i1 %v28, label %bb_then_5_3, label %bb_if_end_4_4
bb_then_5_3:
  %v29 = call i64 @"alpine::draw_log"(i64 %v1, i64 40, i64 80, ptr @.str.1, i64 %v9)
  br label %bb_if_end_4_4
bb_if_end_4_4:
  %v30 = icmp sgt i64 %v21, 50
  %v31 = zext i1 %v30 to i64
  %v32 = icmp ne i64 %v31, 0
  br i1 %v32, label %bb_then_7_5, label %bb_if_end_6_6
bb_then_7_5:
  %v33 = call i64 @"alpine::draw_log"(i64 %v1, i64 40, i64 100, ptr @.str.2, i64 %v9)
  br label %bb_if_end_6_6
bb_if_end_6_6:
  %v34 = icmp sgt i64 %v21, 70
  %v35 = zext i1 %v34 to i64
  %v36 = icmp ne i64 %v35, 0
  br i1 %v36, label %bb_then_9_7, label %bb_if_end_8_8
bb_then_9_7:
  %v37 = call i64 @"alpine::draw_log"(i64 %v1, i64 40, i64 120, ptr @.str.3, i64 %v9)
  br label %bb_if_end_8_8
bb_if_end_8_8:
  %v38 = icmp sgt i64 %v21, 90
  %v39 = zext i1 %v38 to i64
  %v40 = icmp ne i64 %v39, 0
  br i1 %v40, label %bb_then_11_9, label %bb_if_end_10_10
bb_then_11_9:
  %v41 = call i64 @"alpine::draw_log"(i64 %v1, i64 40, i64 140, ptr @.str.4, i64 %v7)
  br label %bb_if_end_10_10
bb_if_end_10_10:
  %v42 = icmp sgt i64 %v21, 110
  %v43 = zext i1 %v42 to i64
  %v44 = icmp ne i64 %v43, 0
  br i1 %v44, label %bb_then_13_11, label %bb_if_end_12_12
bb_then_13_11:
  %v45 = call i64 @"alpine::draw_log"(i64 %v1, i64 40, i64 160, ptr @.str.5, i64 %v7)
  br label %bb_if_end_12_12
bb_if_end_12_12:
  %v46 = icmp sgt i64 %v21, 130
  %v47 = zext i1 %v46 to i64
  %v48 = icmp ne i64 %v47, 0
  br i1 %v48, label %bb_then_15_13, label %bb_if_end_14_14
bb_then_15_13:
  %v49 = add i64 0, 1
  %v50 = call i64 @"alpine::draw_log"(i64 %v1, i64 40, i64 200, ptr @.str.6, i64 %v8)
  %v51 = call i64 @"alpine::draw_rect"(i64 %v1, i64 210, i64 200, i64 10, i64 16, i64 4294967295)
  br label %bb_if_end_14_14
bb_if_end_14_14:
  %v52 = inttoptr i64 %v3 to i32*
  %v53 = load volatile i32, i32* %v52
  %v54 = zext i32 %v53 to i64
  %v55 = add i64 0, %v54
  %v56 = add i64 %v3, 4
  %v57 = inttoptr i64 %v56 to i32*
  %v58 = load volatile i32, i32* %v57
  %v59 = zext i32 %v58 to i64
  %v60 = add i64 0, %v59
  %v61 = add i64 %v3, 8
  %v62 = inttoptr i64 %v61 to i32*
  %v63 = load volatile i32, i32* %v62
  %v64 = zext i32 %v63 to i64
  %v65 = add i64 0, %v64
  %v66 = sub i64 %v55, 2
  %v67 = sub i64 %v60, 2
  %v68 = call i64 @"alpine::draw_rect"(i64 %v1, i64 %v66, i64 %v67, i64 4, i64 4, i64 %v8)
  call void asm sideeffect "hlt", "~{dirflag},~{fpsr},~{flags}"()
  br label %bb_loop_0_0, !llvm.loop !0
bb_loop_end_1_15:
  ret i32 0
}

define i64 @"alpine::draw_log"(i64 %arg0, i64 %arg1, i64 %arg2, i64 %arg3, i64 %arg4) {
entry:
  %v0 = add i64 0, 300
  %v1 = add i64 %arg2, 14
  %v2 = call i64 @"alpine::draw_rect"(i64 %arg0, i64 %arg1, i64 %v1, i64 %v0, i64 1, i64 4281545523)
  %v3 = call i64 @"alpine::draw_rect"(i64 %arg0, i64 %arg1, i64 %arg2, i64 4, i64 12, i64 %arg4)
  ret i64 0
}

define i64 @"alpine::draw_alpine_logo"(i64 %arg0, i64 %arg1, i64 %arg2) {
entry:
  %v0 = add i64 %arg1, 40
  %v1 = add i64 %arg2, 0
  %v2 = call i64 @"alpine::draw_rect"(i64 %arg0, i64 %v0, i64 %v1, i64 2, i64 80, i64 4294967295)
  %v3 = add i64 %arg1, 80
  %v4 = add i64 %arg2, 0
  %v5 = call i64 @"alpine::draw_rect"(i64 %arg0, i64 %v3, i64 %v4, i64 2, i64 80, i64 4294967295)
  %v6 = add i64 %arg1, 40
  %v7 = add i64 %arg2, 40
  %v8 = call i64 @"alpine::draw_rect"(i64 %arg0, i64 %v6, i64 %v7, i64 40, i64 2, i64 4294967295)
  %v9 = add i64 %arg1, 38
  %v10 = sub i64 %arg2, 2
  %v11 = call i64 @"alpine::draw_rect"(i64 %arg0, i64 %v9, i64 %v10, i64 44, i64 84, i64 855700223)
  ret i64 0
}

define i64 @"alpine::draw_rect"(i64 %arg0, i64 %arg1, i64 %arg2, i64 %arg3, i64 %arg4, i64 %arg5) {
entry:
  %v0 = add i64 0, 0
  br label %bb_while_cond_16_0, !llvm.loop !0
bb_while_cond_16_0:
  %v1 = icmp slt i64 %v0, %arg4
  %v2 = zext i1 %v1 to i64
  %v3 = icmp ne i64 %v2, 0
  br i1 %v3, label %bb_while_body_17_1, label %bb_while_end_18_7
bb_while_body_17_1:
  %v4 = add i64 0, 0
  br label %bb_while_cond_19_2, !llvm.loop !0
bb_while_cond_19_2:
  %v5 = icmp slt i64 %v4, %arg3
  %v6 = zext i1 %v5 to i64
  %v7 = icmp ne i64 %v6, 0
  br i1 %v7, label %bb_while_body_20_3, label %bb_while_end_21_6
bb_while_body_20_3:
  %v8 = add i64 %arg2, %v0
  %v9 = add i64 0, %v8
  %v10 = add i64 %arg1, %v4
  %v11 = add i64 0, %v10
  %v12 = icmp sge i64 %v11, 0
  %v13 = zext i1 %v12 to i64
  %v14 = icmp slt i64 %v11, 800
  %v15 = zext i1 %v14 to i64
  %v16 = icmp ne i64 %v13, 0
  %v17 = icmp ne i64 %v15, 0
  %v18 = and i1 %v16, %v17
  %v19 = zext i1 %v18 to i64
  %v20 = icmp sge i64 %v9, 0
  %v21 = zext i1 %v20 to i64
  %v22 = icmp ne i64 %v19, 0
  %v23 = icmp ne i64 %v21, 0
  %v24 = and i1 %v22, %v23
  %v25 = zext i1 %v24 to i64
  %v26 = icmp slt i64 %v9, 600
  %v27 = zext i1 %v26 to i64
  %v28 = icmp ne i64 %v25, 0
  %v29 = icmp ne i64 %v27, 0
  %v30 = and i1 %v28, %v29
  %v31 = zext i1 %v30 to i64
  %v32 = icmp ne i64 %v31, 0
  br i1 %v32, label %bb_then_23_4, label %bb_if_end_22_5
bb_then_23_4:
  %v33 = mul i64 %v9, 800
  %v34 = add i64 %v33, %v11
  %v35 = mul i64 %v34, 4
  %v36 = add i64 %arg0, %v35
  %v37 = inttoptr i64 %v36 to i32*
  %v38 = trunc i64 %arg5 to i32
  store volatile i32 %v38, i32* %v37
  br label %bb_if_end_22_5
bb_if_end_22_5:
  %v39 = add i64 %v4, 1
  %v40 = add i64 0, %v39
  br label %bb_while_cond_19_2, !llvm.loop !0
bb_while_end_21_6:
  %v41 = add i64 %v0, 1
  %v42 = add i64 0, %v41
  br label %bb_while_cond_16_0, !llvm.loop !0
bb_while_end_18_7:
  ret i64 0
}


!0 = !{!0, !1}
!1 = !{!"llvm.loop.vectorize.enable", i1 true}
