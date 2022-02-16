import asyncio
import websockets
import os
import msgpack


OUTPUT_CHUCK = 100000

async def main():
    async with websockets.connect(f"ws://{os.environ['CS5223FET_HOST']}/websocket") as websocket:
        print(websocket)
        while True:
            to_worker = msgpack.loads(await websocket.recv())
            print(to_worker['command'])
            with open('submit.tar.gz', 'wb') as submit_file:
                submit_file.write(bytes(to_worker['upload']))

            proc = await asyncio.create_subprocess_shell(to_worker['command'],
                stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.STDOUT)
            
            async def reader(proc):
                output = []
                while True:
                    chuck = await proc.stdout.read(OUTPUT_CHUCK)
                    print(chuck)
                    if not chuck:
                        return ''.join(output)
                    chuck_length = len(chuck)
                    output.append(chuck.decode())
                    output = output[-2:]
            
            output_task = asyncio.create_task(reader(proc))
            
            is_timeout = False
            try:
                await asyncio.wait_for(proc.wait(), to_worker['timeout'])
            except asyncio.TimeoutError:
                is_timeout = True
                proc.terminate()
            
            output = await output_task
            print(f'output length: {len(output)}')
            if is_timeout:
                output += '\n*** Terminated on hard timeout.'

            from_worker = {
                'task_id': to_worker['task_id'],
                'output': output,
            }
            await websocket.send(msgpack.dumps(from_worker))

asyncio.run(main())