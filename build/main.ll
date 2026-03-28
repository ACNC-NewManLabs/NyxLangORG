target triple = "x86_64-unknown-linux-gnu"
@.fmt_i64 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.fmt_f64 = private unnamed_addr constant [4 x i8] c"%f\0A\00"
@.fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00"
@.str.0 = private unnamed_addr constant [15 x i8] c"Hello, World!\0A\00"
declare i32 @printf(ptr noundef, ...)
declare ptr @malloc(i64)

define i32 @main() {
entry:
  %v0 = bitcast ptr @.str.0 to ptr
  %p1 = call i32 (ptr, ...) @printf(ptr noundef @.fmt_str, ptr noundef %v0)
  ret i32 0
}

