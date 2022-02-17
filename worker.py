import asyncio
import websockets
import os
import signal
import msgpack
import collections

OUTPUT_CHUCK = 50000000

async def main():
    async with websockets.connect(f"ws://{os.environ['CS5223FET_HOST']}/websocket") as websocket:
        print(websocket)
        while True:
            to_worker = msgpack.loads(await websocket.recv(), raw=False)
            print(f'Task #{to_worker["task_id"]}', to_worker['command'])
            with open('submit.tar.gz', 'wb') as submit_file:
                submit_file.write(bytes(to_worker['upload']))

            proc = await asyncio.create_subprocess_shell(to_worker['command'],
                stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.STDOUT,
                preexec_fn=os.setsid)
            
            async def reader(proc):
                output = collections.deque()
                output_length = 0
                while True:
                    chuck = await proc.stdout.read(OUTPUT_CHUCK)
                    if not chuck:
                        return ''.join(output)
                    output.append(chuck.decode())
                    output_length += len(chuck)
                    while output_length - len(output[0]) >= OUTPUT_CHUCK:
                        discard = output.popleft()
                        output_length -= len(discard)
            
            output_task = asyncio.create_task(reader(proc))
            
            is_timeout = False
            try:
                await asyncio.wait_for(proc.wait(), to_worker['timeout'])
            except asyncio.TimeoutError:
                is_timeout = True
                print(f'kill {proc}')
                os.killpg(os.getpgid(proc.pid), signal.SIGTERM)
            
            output = await output_task
            print(f'output length: {len(output)}')
            if is_timeout:
                output += '\n*** Terminated on hard timeout.'

            from_worker = {
                'task_id': to_worker['task_id'],
                'output': output,
            }
            await websocket.send(msgpack.dumps(from_worker, use_bin_type=True))

asyncio.run(main())