import asyncio
import websockets
import json
import subprocess
import threading

async def main():
    async with websockets.connect("ws://localhost:8080/websocket") as websocket:
        print(websocket)
        while True:
            to_worker = json.loads(await websocket.recv())
            print(to_worker['command'])

            signal = asyncio.Future()
            def work_thread():
                p = subprocess.run(to_worker['command'], shell=True, capture_output=True, text=True)
                signal.set_result(p.stdout)
            threading.Thread(target=work_thread).start()

            output = await signal
            from_worker = {
                "task_id": to_worker["task_id"],
                "output": output,
            }
            print(from_worker)
            await websocket.send(json.dumps(from_worker))

asyncio.run(main())