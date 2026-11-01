"""reverse.py UDID PORT [PORT ...] — `adb reverse` for an iPhone, through the hand.

The device-test hand (VelaWalletUITests/DeviceHandUITests, started with
TEST_RUNNER_VELA_DEVICE_HAND_REVERSE="PORT,...") listens on the phone for the
app on PORT and for this script on PORT+20000. This script keeps a few idle
connections to PORT+20000 open through `iproxy` (usbmux, the cable); when the
hand pairs one with an app connection it sends one byte, and this script
connects it to 127.0.0.1:PORT on the Mac. So the app's http://127.0.0.1:PORT
is the Mac's service, as `adb reverse tcp:PORT tcp:PORT` makes it on Android.
"""
import asyncio
import subprocess
import sys

IDLE = 4


async def pipe(reader, writer):
    try:
        while True:
            data = await reader.read(65536)
            if not data:
                break
            writer.write(data)
            await writer.drain()
    except Exception:
        pass
    finally:
        try:
            writer.close()
        except Exception:
            pass


async def one(port):
    """One idle connection: wait for the hand's byte, then carry it to the Mac's service."""
    try:
        r, w = await asyncio.open_connection("127.0.0.1", port + 20000)
        signal = await r.readexactly(1)
    except Exception:
        await asyncio.sleep(1)
        return
    if signal != b"\x01":
        w.close()
        return
    try:
        sr, sw = await asyncio.open_connection("127.0.0.1", port)
    except Exception:
        w.close()
        return
    asyncio.ensure_future(asyncio.gather(pipe(r, sw), pipe(sr, w)))


async def keep(port):
    """Keep IDLE connections waiting for this port."""
    pending = set()
    while True:
        while len(pending) < IDLE:
            pending.add(asyncio.ensure_future(one(port)))
        done, pending = await asyncio.wait(pending, return_when=asyncio.FIRST_COMPLETED)


async def main():
    udid, ports = sys.argv[1], [int(p) for p in sys.argv[2:]]
    proxy = subprocess.Popen(
        ["iproxy", "-u", udid, *[f"{p + 20000}:{p + 20000}" for p in ports]],
        stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL,
    )
    try:
        await asyncio.sleep(1)
        print(f"reverse: {', '.join(str(p) for p in ports)} on the iPhone -> this Mac", flush=True)
        await asyncio.gather(*(keep(p) for p in ports))
    finally:
        proxy.terminate()


asyncio.run(main())
