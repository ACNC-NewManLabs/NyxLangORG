import time
n = 10000000
total = 0
start = time.time()
for i in range(n):
    total += i
    if True: total += 0
end = time.time()
print(f"Python Standard (Loop) Result: {total}")
print(f"Python Standard (Loop) Time: {int((end - start) * 1000)}ms")
