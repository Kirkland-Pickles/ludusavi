#!/usr/bin/env python3
"""Check Windows console inheritance, CLI input/output, and direct GUI launches."""

import argparse
import ctypes
from ctypes import wintypes
import json
import os
from pathlib import Path
import shlex
import shutil
import subprocess
import sys
import tempfile
import time


kernel = ctypes.WinDLL("kernel32", use_last_error=True)
kernel.AttachConsole.argtypes = [wintypes.DWORD]
kernel.AttachConsole.restype = wintypes.BOOL


def configuration(binary, root):
    folder = root / "config"
    folder.mkdir()
    command = [str(binary), "--config", str(folder), "--no-manifest-update"]
    config = json.loads(subprocess.check_output(command + ["config", "show", "--api"], text=True,
                                               stdin=subprocess.DEVNULL, stderr=subprocess.PIPE))
    save = root / "save.txt"
    save.write_text("original save", encoding="utf-8")
    (folder / "cache.yaml").write_text(json.dumps({"roots": config["roots"]}), encoding="utf-8")
    config.update({
        "roots": [], "release": {"check": False}, "cloud": {"synchronize": False},
        "backup": {"path": str(root / "backups")},
        "restore": {"path": str(root / "backups")},
        "customGames": [{"name": "Launch Test", "files": [str(save)]}],
    })
    (folder / "config.yaml").write_text(json.dumps(config), encoding="utf-8")
    (folder / "manifest.yaml").write_text("{}", encoding="utf-8")
    return command, save


def terminal(binary):
    processes = (wintypes.DWORD * 8)()
    assert kernel.GetConsoleProcessList(processes, len(processes)), "Test harness has no console"
    with tempfile.TemporaryDirectory(prefix="ludusavi-cli-test-") as temporary:
        root = Path(temporary)
        isolated_binary = root / "ludusavi.exe"
        shutil.copyfile(binary, isolated_binary)
        (root / "ludusavi.portable").touch()
        binary = isolated_binary
        command, save = configuration(binary, root)
        assert "Ludusavi" in subprocess.check_output(command + ["--help"], text=True)
        invalid = subprocess.run(command + ["--invalid-option"], capture_output=True, text=True)
        assert invalid.returncode != 0 and "error:" in invalid.stderr
        preview = command + ["backup", "--preview", "--api", "Launch Test"]
        cmd = subprocess.list2cmdline(preview) + " & echo __AFTER__"
        powershell = "& " + " ".join("'" + arg.replace("'", "''") + "'" for arg in preview)
        powershell += "; if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }; '__AFTER__'"
        shells = [
            ("CMD", subprocess.list2cmdline([os.environ["COMSPEC"]]) + ' /d /s /c "' + cmd + '"'),
            ("PowerShell", ["powershell.exe", "-NoProfile", "-Command", powershell]),
        ]
        bash = Path(os.environ.get("ProgramFiles", "C:/Program Files")) / "Git/bin/bash.exe"
        if bash.exists():
            bash_command = " ".join(shlex.quote(arg.replace("\\", "/")) for arg in preview)
            shells.append(("Git Bash", [str(bash), "--noprofile", "--norc", "-c",
                                        bash_command + "; result=$?; echo __AFTER__; exit $result"]))
        for name, invocation in shells:
            result = subprocess.run(invocation, capture_output=True, text=True, timeout=30)
            assert result.returncode == 0, f"{name}: {result.stdout}\n{result.stderr}"
            assert not result.stderr.strip(), f"{name}: {result.stderr}"
            output, marker = result.stdout.rsplit("__AFTER__", 1)
            assert not marker.strip(), f"{name} returned before the CLI output finished"
            assert "Launch Test" in json.loads(output)["games"]
            print(f"{name}: CLI output and shell waiting passed")

        redirected = root / "redirected.json"
        invocation = subprocess.list2cmdline(preview) + " > " + subprocess.list2cmdline([str(redirected)])
        result = subprocess.run(subprocess.list2cmdline([os.environ["COMSPEC"]]) + ' /d /s /c "' + invocation + '"',
                                capture_output=True, text=True, timeout=30)
        assert result.returncode == 0 and not result.stdout.strip(), result.stderr
        assert "Launch Test" in json.loads(redirected.read_text(encoding="utf-8"))["games"]
        pipeline = powershell.split("; if ($LASTEXITCODE", 1)[0] + " | ConvertFrom-Json | ForEach-Object { $_.games.PSObject.Properties.Name }"
        result = subprocess.run(["powershell.exe", "-NoProfile", "-Command", pipeline], capture_output=True, text=True, timeout=30)
        assert result.returncode == 0 and "Launch Test" in result.stdout, result.stderr
        print("Shell file redirection and JSON pipeline passed")

        # Confirm a real backup using console keyboard events rather than --force.
        class KeyEvent(ctypes.Structure):
            _fields_ = [("down", wintypes.BOOL), ("repeat", wintypes.WORD),
                        ("key", wintypes.WORD), ("scan", wintypes.WORD),
                        ("character", wintypes.WCHAR), ("control", wintypes.DWORD)]

        class Event(ctypes.Union):
            _fields_ = [("key", KeyEvent), ("padding", ctypes.c_byte * 16)]

        class InputRecord(ctypes.Structure):
            _fields_ = [("type", wintypes.WORD), ("event", Event)]

        kernel.WriteConsoleInputW.argtypes = [wintypes.HANDLE, ctypes.POINTER(InputRecord),
                                             wintypes.DWORD, ctypes.POINTER(wintypes.DWORD)]
        kernel.CreateFileW.argtypes = [wintypes.LPCWSTR, wintypes.DWORD, wintypes.DWORD,
                                      wintypes.LPVOID, wintypes.DWORD, wintypes.DWORD, wintypes.HANDLE]
        kernel.CreateFileW.restype = wintypes.HANDLE
        kernel.CloseHandle.argtypes = [wintypes.HANDLE]
        console_input = kernel.CreateFileW("CONIN$", 0xC0000000, 3, None, 3, 0, None)
        assert console_input != wintypes.HANDLE(-1).value
        console_stdin = open("CONIN$", "rb")
        console_stdout = open("CONOUT$", "wb")
        app = subprocess.Popen(command + ["backup", "Launch Test"],
                               stdin=console_stdin, stdout=console_stdout, stderr=console_stdout)
        try:
            time.sleep(1)
            records = (InputRecord * 2)()
            for record, character, key in zip(records, ("y", "\r"), (0x59, 0x0D)):
                record.type = 1
                record.event.key = KeyEvent(True, 1, key, 0, character, 0)
            written = wintypes.DWORD()
            assert kernel.WriteConsoleInputW(console_input, records, 2, ctypes.byref(written)), ctypes.get_last_error()
            assert app.wait(timeout=30) == 0, "Interactive CLI backup failed"
            assert list((root / "backups").glob("*/mapping.yaml")), "Interactive backup was not saved"
        finally:
            kernel.CloseHandle(console_input)
            console_stdin.close()
            console_stdout.close()
            if app.poll() is None:
                app.kill()
                app.wait()
        save.write_text("changed after backup", encoding="utf-8")
        subprocess.run(command + ["restore", "--force", "Launch Test"], capture_output=True, check=True)
        assert save.read_text(encoding="utf-8") == "original save"
        print("Interactive confirmation, backup, restore, pipes, and error exit status passed")


def direct(binary):
    kernel.FreeConsole()
    startup = subprocess.STARTUPINFO()
    startup.dwFlags = subprocess.STARTF_USESHOWWINDOW
    startup.wShowWindow = 0
    # This control proves AttachConsole detects a newly allocated console.
    control = subprocess.Popen([sys.executable, "-c", "import time; time.sleep(30)"],
                               creationflags=subprocess.CREATE_NEW_CONSOLE, startupinfo=startup)
    try:
        time.sleep(1)
        assert kernel.AttachConsole(control.pid), "Console detection control failed"
        kernel.FreeConsole()
    finally:
        control.kill()
        control.wait()
    with tempfile.TemporaryDirectory(prefix="ludusavi-launch-test-") as temporary:
        command, _ = configuration(binary, Path(temporary))
        for suffix, keep_console in (([], True), (["gui"], True), ([], False), (["gui"], False)):
            # Keep any allocated console attached so we can detect it even after startup.
            environment = dict(os.environ)
            environment.pop("LUDUSAVI_DEBUG", None)
            if keep_console:
                environment["LUDUSAVI_DEBUG"] = "1"
            app = subprocess.Popen(command + suffix, env=environment,
                                   startupinfo=startup,
                                   stdin=subprocess.DEVNULL, stdout=subprocess.DEVNULL, stderr=subprocess.DEVNULL)
            try:
                for _ in range(30):
                    assert app.poll() is None, "GUI exited during direct launch"
                    attached = kernel.AttachConsole(app.pid)
                    if attached:
                        kernel.FreeConsole()
                    if sys.getwindowsversion().build >= 26100 or not keep_console:
                        assert not attached, "Direct GUI launch kept a console"
                    time.sleep(0.1)
                print(f"Direct launch ({'gui' if suffix else 'no subcommand'}, debug={keep_console}): console behavior passed")
            finally:
                app.kill()
                app.wait()


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("binary", type=Path)
    parser.add_argument("--terminal", action="store_true", help=argparse.SUPPRESS)
    args = parser.parse_args()
    binary = args.binary.resolve(strict=True)
    if args.terminal:
        terminal(binary)
    else:
        startup = subprocess.STARTUPINFO()
        startup.dwFlags = subprocess.STARTF_USESHOWWINDOW
        startup.wShowWindow = 0
        result = subprocess.run([sys.executable, str(Path(__file__).resolve()), str(binary), "--terminal"],
                                creationflags=subprocess.CREATE_NEW_CONSOLE, startupinfo=startup,
                                capture_output=True, text=True)
        print(result.stdout, end="")
        if result.returncode:
            sys.exit(result.stderr)
        direct(binary)
