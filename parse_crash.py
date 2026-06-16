import json, sys
with open(sys.argv[1]) as f:
    f.readline()
    data = json.loads(f.read())
    idx = data.get("faultingThread")
    thread = data["threads"][idx]
    print(f"Thread {idx} (ID: {thread.get('id')}) crashed!")
    for frame in thread.get("frames", []):
        print(f"[{frame.get('imageOffset', 0):x}] {frame.get('symbol', '?')}")
