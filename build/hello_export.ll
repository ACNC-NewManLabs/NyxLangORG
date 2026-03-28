target triple = "x86_64-unknown-linux-gnu"
@.fmt_i64 = private unnamed_addr constant [6 x i8] c"%lld\0A\00"
@.fmt_f64 = private unnamed_addr constant [4 x i8] c"%f\0A\00"
@.fmt_str = private unnamed_addr constant [4 x i8] c"%s\0A\00"
@.str.0 = private unnamed_addr constant [28 x i8] c"Hello from exported binary!\00"
declare i32 @printf(ptr noundef, ...)
declare ptr @malloc(i64)

declare i64 @println(...)
define i32 @main() {
entry:
  %v0 = call i64 @println(ptr @.str.0)
  ret i32 0
}

