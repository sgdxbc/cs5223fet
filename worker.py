import asyncio
import websockets
import json

async def main():
    async with websockets.connect("ws://localhost:8080/websocket") as websocket:
        print(websocket)
        while True:
            to_worker = json.loads(await websocket.recv())
            print(to_worker['command'])
            sleep_lit, second = to_worker['command'].split()
            second = int(second)
            await asyncio.sleep(second)
            
            result = f'finished task #{to_worker["task_id"]}'
            print(result)
            from_worker = {
                "task_id": to_worker["task_id"],
                "result": result
            }
            await websocket.send(json.dumps(from_worker))

asyncio.run(main())