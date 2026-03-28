target triple = "x86_64-unknown-linux-gnu"
@.fmt = private unnamed_addr constant [4 x i8] c"%d\0A\00"
declare i32 @printf(ptr noundef, ...)

define i32 @main() {
entry:
  %v0 = call i64 @println(i64 0)
  %v1 = call i64 @native_renderer_init(i64 0, i64 1, i64 1)
  %v2 = icmp ne i64 %v1, 0
  br i1 %v2, label %.then_1, label %.else_2
.then_1:
  %v3 = call i64 @println(i64 0)
  %v4 = call i64 @println_no_args()
  %v5 = call i64 @native_renderer_shutdown()
  br label %.if_end_3
.else_2:
  %v6 = call i64 @println(i64 0)
  br label %.if_end_3
.if_end_3:
  ret i32 0
}

declare i64 @println(i64)
declare i64 @println_no_args()
declare i64 @native_renderer_init(i64, i64, i64)
declare void @native_renderer_shutdown()
