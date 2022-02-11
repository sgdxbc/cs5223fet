import asyncio
import websockets

async def main():
    async with websockets.connect("ws://localhost:8080/websocket") as websocket:
        print(websocket)
        while True:
            task_command = await websocket.recv()
            print(task_command)
            sleep_lit, second = task_command.split()
            second = int(second)
            asyncio.sleep(second)
            
            result = f'finished: {task_command}'
            await websocket.send(result)

asyncio.run(main())