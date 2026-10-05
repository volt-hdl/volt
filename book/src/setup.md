# Setup

<div class="chapter-goal">

In this chapter you will open a terminal, make a folder for your Volt
projects, pick an editor and learn three ways to create a file. Then you
will install Volt and Docker Desktop and check that both work. If you use
a terminal every day, skim the headings and do the last three sections.

</div>

<div class="before-you-start">

**Before you start**

- **Window:** none yet; this page shows how to open a terminal.
- **Folder:** none yet; this page makes one.
- **Running:** a web browser with this book, and an internet connection.

</div>

## Volt is a command-line tool

Volt has no window of its own. You use it in two programs side by side:

- a **text editor**, where you write designs: files whose names end in
  `.volt`;
- a **terminal**, where you type commands such as `volt check` and `volt
  test`. Volt reads your files, prints what it found, and writes its
  results into a folder named `build`.

## Open a terminal

A terminal is a window where you type a command and press Enter, and the
command prints its answer below it.

- **Windows:** open the Start menu, type `PowerShell` and press Enter. A
  window opens with a line like `PS C:\Users\you>`. Windows 11 may show it
  inside the *Terminal* app; that is the same thing. Use PowerShell, not
  *Command Prompt*: the Windows commands in this book are PowerShell
  commands.
- **macOS:** press Cmd+Space, type `Terminal` and press Enter.
- **Linux:** press Ctrl+Alt+T, or search for *Terminal* in your
  applications.

The text before the cursor (`PS C:\Users\you>` on Windows, something
ending in `%` or `$` on macOS and Linux) is the *prompt*. It is not part
of any command, and the commands in this book never include it.

### The three kinds of blocks in this book

The guided pages (this one and the Tour) label every block:

**Type this:** a command. Copy it with the button in the block's top
right corner, paste it into the terminal (Windows: right-click or Ctrl+V;
macOS: Cmd+V; Linux: Ctrl+Shift+V) and press Enter. Or type it.

```console
volt --version
```

**You should see:** what the command prints. These blocks have no copy
button: you compare them with your terminal, you do not type them. Small
differences, such as times like `0.01s`, do not matter.

```text,output
volt 0.1.0
```

**Create this file:** `name.volt`, followed by the file's contents. The
section [Create a file](#create-a-file) below shows how.

When Windows and macOS or Linux need different commands, the book shows
both, each with its own label: **Type this (Windows, PowerShell):** and
**Type this (macOS and Linux):**, for example. Use the one for your
computer.

## Make a folder for your projects

A terminal always works *in* a folder, its *working folder*. A new
terminal starts in your home folder (on Windows `C:\Users\<your name>`).
Commands such as `volt new` create their files in the working folder.

Make a folder named `volt-projects` in your home folder. You do this once.

**Type this:**

```console
mkdir ~/volt-projects
```

`~` means your home folder; it works in PowerShell, on macOS and on
Linux. PowerShell answers with a short table that names the new folder;
macOS and Linux print nothing.

Now go into it. `cd` means *change directory*:

**Type this:**

```console
cd ~/volt-projects
```

On Windows the prompt now ends in `volt-projects>`. On every system, `pwd`
prints the working folder. `cd ..` goes up one folder; `cd ~/volt-projects`
brings you back here from anywhere. Every Tour page starts with a `cd`
command like this one, in its **Before you start** box, so you always know
where you are.

To see the folder in your file manager:

**Type this (Windows, PowerShell):**

```powershell
explorer .
```

**Type this (macOS):**

```sh
open .
```

**Type this (Linux):**

```sh
xdg-open .
```

`.` means the working folder.

## Pick an editor

Any editor that saves plain text works.

- **[Visual Studio Code](https://code.visualstudio.com/)** (VS Code) is
  free and runs on Windows, macOS and Linux. We suggest it: it shows the
  folder and the terminal in the same window (menu **Terminal → New
  Terminal**). Open your project with **File → Open Folder**.
- **Notepad** (Windows) works too. Read the warning about `.txt` below.
- On macOS, TextEdit saves formatted text by default; choose **Format →
  Make Plain Text** before you save, or use VS Code.

## Create a file

A guided page shows a file like this:

**Create this file:** `hello.volt`

```volt,file=hello.volt
// A wire from one port to another.
pub module Hello {
    in  a : bool
    out b : bool

    b = a
}
```

The file goes into the working folder named in the page's **Before you
start** box. There are three ways to create it. Try one now with
`hello.volt`, in `volt-projects`.

### With the clipboard

Click the copy button of the file's block. Then, in the terminal:

**Type this (Windows, PowerShell):**

```powershell
Get-Clipboard -Raw | Set-Content hello.volt -NoNewline
```

**Type this (macOS):**

```sh
pbpaste > hello.volt
```

**Type this (Linux):**

```sh
wl-paste > hello.volt
```

`wl-paste` comes with the `wl-clipboard` package. On an older X11 desktop
use `xclip -selection clipboard -o > hello.volt` (package `xclip`).

These commands write the file in the working folder and replace a file of
the same name if there is one. Put the name of the file you are creating
in place of `hello.volt`.

### With VS Code

Open the folder (**File → Open Folder**), then **File → New File**, type
the name `hello.volt` and press Enter. Paste the contents and save with
Ctrl+S (macOS: Cmd+S).

### With Notepad

Open Notepad and paste the contents. Choose **File → Save as**, go to the
folder (`volt-projects` is in your user folder, `C:\Users\<your name>`),
and set **Save as type** to **All files (\*.\*)**. Type the name
`hello.volt` and save.

<div class="box new-to-hw">

**New to files?** The `.txt` trap

With **Save as type** left at *Text Documents (\*.txt)*, Notepad saves
`hello.volt.txt`. File Explorer hides the `.txt` part, so the file looks
right, and then Volt cannot find `hello.volt`. Typing the name in quotes,
`"hello.volt"`, also stops Notepad from adding `.txt`.

</div>

### Check that the file is there

`ls` lists the files of the working folder, with their full names:

**Type this:**

```console
ls
```

You should find `hello.volt` in the list, not `hello.volt.txt`.

### Replace a file

Some pages change a file you made earlier and say **Replace this file:**
followed by the name. Use the clipboard command again (it replaces the
file), or in the editor select everything (Ctrl+A, macOS: Cmd+A), paste
the new contents and save.

## Install Volt

**Type this (Windows, PowerShell):**

```powershell
irm https://volt-hdl.github.io/volt/install.ps1 | iex
```

**Type this (macOS and Linux):**

```sh
curl -fsSL https://volt-hdl.github.io/volt/install.sh | sh
```

The script prints each step. When it has finished, **close the terminal
and open a new one**: a terminal that was already open does not know the
new `volt` command. If you use the Windows *Terminal* app or VS Code, close
the whole program, not just the tab.

If the script says "No Volt release has been published yet.", there is
no prebuilt `volt` to download yet: build it from source as
[Install Volt](tour/install.md#build-from-source) shows. That page also
explains what the script changes and how to update or remove Volt.

## Check Volt

In the new terminal:

**Type this:**

```console
volt --version
```

**You should see:**

```text,output
volt 0.1.0
```

This book describes Volt 0.1.0. The book at
<https://volt-hdl.github.io/volt/> follows the latest release, and the
book at <https://volt-hdl.github.io/volt/dev/> follows the development
version. If `volt --version` prints another version, some output on these
pages may differ from yours.

If you see "The term 'volt' is not recognized" (Windows) or "command not
found" (macOS, Linux), the terminal was opened before the install
finished: close it, and the program around it, and open a new one.

## Install and start Docker Desktop

`volt check` and `volt build` need nothing but `volt`. `volt test` runs
your design in a simulator called Verilator. If Verilator is not
installed, Volt runs it inside Docker, a program that starts small
prepared Linux systems (*containers*). So for `volt test`:

1. Install [Docker Desktop](https://www.docker.com/products/docker-desktop/)
   (Windows, macOS) or Docker Engine (Linux).
2. Start it. On Windows, open the Start menu and type `Docker Desktop`; on
   macOS, open it from Applications.
3. Wait until the Docker Desktop window says **Engine running** in its
   bottom left corner.

Installed is not enough: Docker Desktop must be **running** whenever you
run `volt test`. If you closed it, start it again and wait for **Engine
running**. Its settings have an option to start it when you sign in.

## Ask `volt doctor`

`volt doctor` checks what works on your computer.

**Type this:**

```console
volt doctor
```

Your own user folder appears where this output says `C:\Users\you`.

**You should see (Windows 11, Docker Desktop running):**

```text,output
volt 0.1.0 (windows-x86_64)

✓ build, check, explain — no external tools needed
✓ test, run — via Docker (verilator/verilator:v5.052: Verilator 5.052, g++ 13.3)
✓ verify — via Docker (hdlc/formal:all: Yosys 0.66, SBY 0.69, boolector 3.2.4, yices 2.7.0, z3 4.15.0)
- timing (optional) — OpenSTA not found; checks generated .sdc files
- driver checks (optional) — C compiler not found (found: rustc 1.95.0); compiles drivers from --emit=c,rust
✓ docker — 29.8.1, daemon running
- project — no Volt.toml (search stopped at the home directory C:\Users\you); single .volt files still work
```

The lines that matter for the Tour are `build, check, explain` and `test,
run`: both need a `✓`. Lines starting with `-` are optional tools.

If Docker Desktop is installed but not running, these lines change:

**You should see (Docker Desktop installed, not running):**

```text,output
✗ test, run — Verilator, C++ compiler, make not found
    fix (Windows): start Docker Desktop: Volt runs Verilator in a container (DOCKER below), or use WSL
    see: volt explain simulation-setup
✗ verify — sby, Yosys not found; no SMT solver found (boolector, bitwuzla, yices, z3)
    fix (Windows): start Docker Desktop: Volt runs sby in a container (DOCKER below), or use WSL
    see: volt explain verify-setup
! docker — 29.8.1 installed, daemon not running (start Docker Desktop or the docker service)
```

Start Docker Desktop, wait for **Engine running**, and run `volt doctor`
again. While Docker Desktop is still starting, the `docker` line may say
`daemon did not answer within 5 s`; wait a little and try again.

## Copy what a command printed

When your terminal shows something other than the **You should see**
block, you may want to compare it closely or ask someone for help. Copy
the text:

- **Windows:** select it with the mouse, then press Enter or Ctrl+C.
  Careful: Ctrl+C with nothing selected stops the running command.
- **macOS:** select it, then Cmd+C.
- **Linux:** select it, then Ctrl+Shift+C.

Or run the command again and send everything it prints straight to the
clipboard:

**Type this (Windows, PowerShell):**

```powershell
volt check 2>&1 | Set-Clipboard
```

**Type this (macOS):**

```sh
volt check 2>&1 | pbcopy
```

**Type this (Linux):**

```sh
volt check 2>&1 | wl-copy
```

`2>&1` takes the error messages along with the rest. Then paste where you
need it.

Next: [Part I, the Tour](tour/new-check-build.md).
