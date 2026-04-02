import time
n = 10000000
total = 0
start = time.time()
for i in range(n):
    total += i
end = time.time()
print(f"Python (Loop) Result: {total}")
print(f"Python (Loop) Time: {int((end - start) * 1000)}ms")
