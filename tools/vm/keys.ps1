# keys.ps1 -Exe <program> [-Open <file> | -ArgLine "<args>"] -Keys "<k1>;<k2>…" [-WaitSec n] [-Shot name.png] [-Leave] [-Soft]
# In the desktop session: start the program (with a file), bring it to the
# front, run each ';'-separated step 1.5 s apart, optionally capture the
# screen, then close it (unless -Leave). A step is SendKeys text, or
# "CLICK x,y" / "RCLICK x,y" / "DCLICK x,y" (screen pixels), "MOVE x,y" (the
# pointer there in five steps, no button), "DRAG x1,y1,x2,y2"
# (a left-button drag in ten steps; "HOLD x1,y1,x2,y2" the same left held,
# then "UP"), "WHEEL x,y,n"
# (n notches, + up), "KEYDOWN ctrl" / "KEYUP ctrl" (shift, alt: a modifier
# held across steps; also space, enter, esc, insert, home, end, pgup, pgdn, the arrows and tab: a key held), "SIZE w,h" (the front window restored, w × h at 0,0),
# "CLOSE" (WM_CLOSE to the front window), "SHOT name.png" (a capture now),
# "WINS name.txt" (the program's top-level windows: handle, owner, visible,
# enabled, class, title — into shots/), "CURSOR name.txt" (the cursor now
# shown, by its IDC_ name when it is a system one — appended to shots/),
# "FOCUS name.txt" (the keyboard focus of the front window's thread: the
# focus window's handle, class, visibility and its ancestors, and its
# untitled children — appended to shots/), "CLIP name.txt" (the clipboard's
# formats now, by number or registered name — appended to shots/), "TREE
# name.txt" (the front window's descendants, indented: class, title, visible,
# enabled, rectangle — into shots/), "FLOATAT x,y" (the first visible
# floating pane's mini-frame moved so that its top-left corner is at x,y,
# its size kept), "CLIPEMF name.emf" (the clipboard's enhanced metafile
# saved into shots/).
# Logs next to itself.
param([Parameter(Mandatory)][string]$Exe, [string]$Open = '', [string]$ArgLine = '', [string]$Keys = '', [int]$WaitSec = 6, [string]$Shot = '', [switch]$Leave, [switch]$Soft, [string]$SoftLog = '')
$here = Split-Path -Parent $MyInvocation.MyCommand.Path
$log = Join-Path $here 'keys.log'
Add-Type @'
using System; using System.Runtime.InteropServices;
public static class In {
  [DllImport("user32.dll")] public static extern bool SetProcessDPIAware();
  [DllImport("user32.dll")] public static extern bool SetCursorPos(int x, int y);
  [DllImport("user32.dll")] public static extern void mouse_event(uint f, uint dx, uint dy, int data, UIntPtr extra);
  [DllImport("kernel32.dll")] public static extern uint SetThreadExecutionState(uint flags);
  [DllImport("user32.dll")] public static extern void keybd_event(byte vk, byte scan, uint flags, UIntPtr extra);
  [DllImport("user32.dll")] public static extern uint MapVirtualKey(uint code, uint type);
  [DllImport("user32.dll")] public static extern IntPtr GetForegroundWindow();
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr h, uint msg, IntPtr w, IntPtr l);
  [DllImport("user32.dll")] public static extern bool ShowWindow(IntPtr h, int cmd);
  [DllImport("user32.dll")] public static extern bool MoveWindow(IntPtr h, int x, int y, int w, int ht, bool repaint);
  public delegate bool EnumProc(IntPtr h, IntPtr l);
  [DllImport("user32.dll")] public static extern bool EnumWindows(EnumProc f, IntPtr l);
  [DllImport("user32.dll")] public static extern uint GetWindowThreadProcessId(IntPtr h, out uint pid);
  [DllImport("user32.dll")] public static extern IntPtr GetWindow(IntPtr h, uint cmd);
  [DllImport("user32.dll")] public static extern bool IsWindowVisible(IntPtr h);
  [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr h);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClassName(IntPtr h, System.Text.StringBuilder s, int n);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetWindowText(IntPtr h, System.Text.StringBuilder s, int n);
  [StructLayout(LayoutKind.Sequential)] public struct CURSORINFO { public int cbSize; public int flags; public IntPtr hCursor; public int x; public int y; }
  [DllImport("user32.dll")] public static extern bool GetCursorInfo(ref CURSORINFO ci);
  [DllImport("user32.dll")] public static extern IntPtr LoadCursor(IntPtr inst, IntPtr id);
  [DllImport("user32.dll")] public static extern IntPtr GetCursor();
  [DllImport("user32.dll")] public static extern bool AttachThreadInput(uint a, uint b, bool attach);
  [DllImport("kernel32.dll")] public static extern uint GetCurrentThreadId();
  [DllImport("user32.dll")] public static extern bool DrawIconEx(IntPtr hdc, int x, int y, IntPtr h, int w, int hh, int step, IntPtr brush, int flags);
  public static string Cursor() {
    var ci = new CURSORINFO(); ci.cbSize = Marshal.SizeOf(typeof(CURSORINFO));
    if (!GetCursorInfo(ref ci)) return "?";
    // With no mouse attached the system hides the cursor (no handle): the
    // front window's thread input still holds the one SetCursor chose.
    if (ci.hCursor == IntPtr.Zero) {
      uint pid; uint t = GetWindowThreadProcessId(GetForegroundWindow(), out pid);
      uint me = GetCurrentThreadId();
      if (AttachThreadInput(me, t, true)) { ci.hCursor = GetCursor(); AttachThreadInput(me, t, false); }
    }
    var names = new System.Collections.Generic.Dictionary<int, string> { {32512, "ARROW"}, {32513, "IBEAM"}, {32514, "WAIT"}, {32515, "CROSS"}, {32516, "UPARROW"}, {32642, "SIZENWSE"}, {32643, "SIZENESW"}, {32644, "SIZEWE"}, {32645, "SIZENS"}, {32646, "SIZEALL"}, {32648, "NO"}, {32649, "HAND"}, {32650, "APPSTARTING"}, {32651, "HELP"} };
    foreach (var kv in names) if (LoadCursor(IntPtr.Zero, (IntPtr)kv.Key) == ci.hCursor) return kv.Value + " at " + ci.x + "," + ci.y;
    return string.Format("{0:X} at {1},{2}", (long)ci.hCursor, ci.x, ci.y);
  }
  [StructLayout(LayoutKind.Sequential)] public struct RECT { public int l, t, r, b; }
  [StructLayout(LayoutKind.Sequential)] public struct GTI { public int cb; public int flags; public IntPtr active, focus, capture, menu, move, caret; public RECT rc; }
  [DllImport("user32.dll")] public static extern bool GetGUIThreadInfo(uint tid, ref GTI gti);
  [DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr h);
  [DllImport("user32.dll")] public static extern bool GetWindowRect(IntPtr h, out RECT r);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern IntPtr FindWindowEx(IntPtr p, IntPtr after, string cls, string title);
  [DllImport("user32.dll")] public static extern bool OpenClipboard(IntPtr h);
  [DllImport("user32.dll")] public static extern bool CloseClipboard();
  [DllImport("user32.dll")] public static extern uint EnumClipboardFormats(uint f);
  [DllImport("user32.dll", CharSet = CharSet.Unicode)] public static extern int GetClipboardFormatName(uint f, System.Text.StringBuilder s, int n);
  static string Desc(IntPtr h) {
    if (h == IntPtr.Zero) return "(none)";
    var c = new System.Text.StringBuilder(256); GetClassName(h, c, 256);
    var t = new System.Text.StringBuilder(256); GetWindowText(h, t, 256);
    return string.Format("{0:X} class={1} title={2} vis={3} en={4}", (long)h, c, t, IsWindowVisible(h), IsWindowEnabled(h));
  }
  [DllImport("user32.dll")] public static extern bool SetWindowPos(IntPtr h, IntPtr after, int x, int y, int w, int ht, uint flags);
  public static string FloatAt(uint pid, int x, int y) {
    string res = "no mini-frame";
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      var c = new System.Text.StringBuilder(256); GetClassName(h, c, 256);
      if (p == pid && c.ToString().StartsWith("Afx:MiniFrame") && IsWindowVisible(h)) {
        RECT r; GetWindowRect(h, out r);
        SetWindowPos(h, IntPtr.Zero, x, y, 0, 0, 0x0001 | 0x0004 | 0x0010);
        GetWindowRect(h, out r);
        res = string.Format("mini-frame now at {0},{1},{2},{3}", r.l, r.t, r.r, r.b);
        return false;
      }
      return true;
    }, IntPtr.Zero);
    return res;
  }
  public static string Focus() {
    var o = new System.Text.StringBuilder();
    IntPtr fg = GetForegroundWindow();
    uint pid; uint tid = GetWindowThreadProcessId(fg, out pid);
    var g = new GTI(); g.cb = Marshal.SizeOf(typeof(GTI));
    if (!GetGUIThreadInfo(tid, ref g)) return "no thread info\n";
    o.AppendFormat("foreground: {0}\nactive: {1}\nfocus: {2}\n", Desc(fg), Desc(g.active), Desc(g.focus));
    for (IntPtr p = GetParent(g.focus); p != IntPtr.Zero; p = GetParent(p)) o.AppendFormat("  parent: {0}\n", Desc(p));
    if (g.focus != IntPtr.Zero) {
      IntPtr c = IntPtr.Zero;
      while ((c = FindWindowEx(g.focus, c, null, "")) != IntPtr.Zero) o.AppendFormat("  untitled child: {0}\n", Desc(c));
    }
    return o.ToString();
  }
  static void Tree(IntPtr h, int depth, System.Text.StringBuilder o) {
    RECT r; GetWindowRect(h, out r);
    o.AppendFormat("{0}{1} [{2},{3},{4},{5}]\n", new string(' ', depth * 2), Desc(h), r.l, r.t, r.r, r.b);
    for (IntPtr c = GetWindow(h, 5); c != IntPtr.Zero; c = GetWindow(c, 2)) Tree(c, depth + 1, o);
  }
  public static string Tree() {
    var o = new System.Text.StringBuilder(); Tree(GetForegroundWindow(), 0, o); return o.ToString();
  }
  [DllImport("user32.dll")] public static extern IntPtr GetClipboardData(uint f);
  [DllImport("gdi32.dll")] public static extern uint GetEnhMetaFileBits(IntPtr h, uint n, byte[] buf);
  public static byte[] ClipEmf() {
    if (!OpenClipboard(IntPtr.Zero)) return null;
    try {
      IntPtr h = GetClipboardData(14);
      if (h == IntPtr.Zero) return null;
      uint n = GetEnhMetaFileBits(h, 0, null);
      var b = new byte[n]; GetEnhMetaFileBits(h, n, b); return b;
    } finally { CloseClipboard(); }
  }
  public static string Clip() {
    if (!OpenClipboard(IntPtr.Zero)) return "clipboard busy\n";
    var o = new System.Text.StringBuilder();
    for (uint f = EnumClipboardFormats(0); f != 0; f = EnumClipboardFormats(f)) {
      var n = new System.Text.StringBuilder(256);
      int k = GetClipboardFormatName(f, n, 256);
      o.AppendFormat("{0} {1}\n", f, k > 0 ? n.ToString() : "");
    }
    CloseClipboard();
    return o.Length == 0 ? "(empty)\n" : o.ToString();
  }
  public static string List(uint pid) {
    var o = new System.Text.StringBuilder();
    EnumWindows((h, l) => {
      uint p; GetWindowThreadProcessId(h, out p);
      if (p == pid) {
        var c = new System.Text.StringBuilder(256); GetClassName(h, c, 256);
        var t = new System.Text.StringBuilder(256); GetWindowText(h, t, 256);
        o.AppendFormat("{0:X} owner={1:X} vis={2} en={3} class={4} title={5}\n", (long)h, (long)GetWindow(h, 4), IsWindowVisible(h), IsWindowEnabled(h), c, t);
      }
      return true;
    }, IntPtr.Zero);
    return o.ToString();
  }
}
'@
[In]::SetProcessDPIAware() | Out-Null
# Keep the display on while the steps run (ES_CONTINUOUS | ES_DISPLAY_REQUIRED):
# an idle VM turns it off and the captures freeze on its last frame.
[In]::SetThreadExecutionState([uint32]2147483650) | Out-Null
function Click([int]$x, [int]$y, [uint32]$down, [uint32]$up, [int]$times = 1, [int]$gap = 60) {
    [In]::SetCursorPos($x, $y) | Out-Null; Start-Sleep -Milliseconds 120
    # A gap other than 60 ms (DCLICK x,y,ms) also holds each press 40 ms, as a hand does.
    $hold = if ($gap -ne 60) { 40 } else { 0 }
    for ($i = 0; $i -lt $times; $i++) { [In]::mouse_event($down, 0, 0, 0, [UIntPtr]::Zero); if ($hold) { Start-Sleep -Milliseconds $hold }; [In]::mouse_event($up, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds $gap }
}
"start $(Get-Date -f s) $Exe $Open" | Out-File $log -Encoding utf8
# -Soft: our build in its software runner from the start (a launcher that
# falls back to it leaves the window to a child process).
if ($Soft) { $env:FV_SOFTWARE = '1' }
# -SoftLog name: the software runner's trace (FV_SOFTLOG) into shots\name.
if ($SoftLog) { $env:FV_SOFTLOG = Join-Path (Join-Path $here 'shots') $SoftLog }
$wsh = New-Object -ComObject WScript.Shell
$p = if ($ArgLine) { Start-Process -FilePath $Exe -ArgumentList $ArgLine -PassThru } elseif ($Open) { Start-Process -FilePath $Exe -ArgumentList "`"$Open`"" -PassThru } else { Start-Process -FilePath $Exe -PassThru }
Start-Sleep -Seconds $WaitSec
$ok = $wsh.AppActivate($p.Id)
"activated $ok pid $($p.Id)" | Out-File $log -Append -Encoding utf8
foreach ($k in ($Keys -split ';')) {
    if (-not $k) { Start-Sleep -Milliseconds 1500; continue }
    if ($k -match '^(R|D)?CLICK (\d+),(\d+)(?:,(\d+))?$') {
        $x = [int]$Matches[2]; $y = [int]$Matches[3]
        switch ($Matches[1]) { 'R' { Click $x $y 0x8 0x10 1 $(if ($Matches[4]) { [int]$Matches[4] } else { 60 }) } 'D' { Click $x $y 0x2 0x4 2 $(if ($Matches[4]) { [int]$Matches[4] } else { 60 }) } default { Click $x $y 0x2 0x4 } }
    } elseif ($k -match '^MOVE (\d+),(\d+)$') {
        $x = [int]$Matches[1]; $y = [int]$Matches[2]
        for ($i = 4; $i -ge 0; $i--) { [In]::SetCursorPos($x - 2 * $i, $y) | Out-Null; Start-Sleep -Milliseconds 40 }
    } elseif ($k -match '^(DRAG|HOLD) (\d+),(\d+),(\d+),(\d+)$') {
        $x1 = [int]$Matches[2]; $y1 = [int]$Matches[3]; $x2 = [int]$Matches[4]; $y2 = [int]$Matches[5]
        [In]::SetCursorPos($x1, $y1) | Out-Null; Start-Sleep -Milliseconds 120
        [In]::mouse_event(0x2, 0, 0, 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 120
        for ($i = 1; $i -le 10; $i++) {
            [In]::SetCursorPos([int]($x1 + ($x2 - $x1) * $i / 10), [int]($y1 + ($y2 - $y1) * $i / 10)) | Out-Null; Start-Sleep -Milliseconds 60
        }
        Start-Sleep -Milliseconds 200
        if ($Matches[1] -eq 'DRAG') { [In]::mouse_event(0x4, 0, 0, 0, [UIntPtr]::Zero) }
    } elseif ($k -match '^KEY(DOWN|UP) (ctrl|shift|alt|space|enter|esc|insert|home|end|pgup|pgdn|up|down|left|right|tab)$') {
        # A modifier held across the next steps (KEYDOWN ctrl; WHEEL …; KEYUP ctrl);
        # a key too (KEYDOWN space; SHOT x.png; KEYUP space: a pushed button's look).
        $v = @{ ctrl = 0x11; shift = 0x10; alt = 0x12; space = 0x20; enter = 0x0D; esc = 0x1B; insert = 0x2D; home = 0x24; end = 0x23; pgup = 0x21; pgdn = 0x22; up = 0x26; down = 0x28; left = 0x25; right = 0x27; tab = 0x09 }[$Matches[2]]
        [In]::keybd_event([byte]$v, [byte][In]::MapVirtualKey($v, 0), $(if ($Matches[1] -eq 'UP') { 2 } else { 0 }), [UIntPtr]::Zero)
    } elseif ($k -match '^CHORD ((?:\w+\+)*)(\w+)$') {
        # Keys with their scan codes (a real keyboard's; SendKeys' Ctrl+letter
        # may come without one): CHORD ctrl+shift+P, CHORD F4, CHORD ctrl+0x4F.
        $mods = @($Matches[1].TrimEnd('+').Split('+') | Where-Object { $_ })
        $name = $Matches[2]
        $vkOf = @{ ctrl = 0x11; shift = 0x10; alt = 0x12 }
        $vk = if ($name -match '^0x[0-9a-fA-F]+$') { [Convert]::ToInt32($name, 16) } elseif ($name.Length -eq 1) { [int][char]$name.ToUpper() } elseif ($name -match '^F(\d+)$') { 0x6F + [int]$Matches[1] } else { 0 }
        foreach ($m in $mods) { $v = $vkOf[$m.ToLower()]; [In]::keybd_event([byte]$v, [byte][In]::MapVirtualKey($v, 0), 0, [UIntPtr]::Zero) }
        [In]::keybd_event([byte]$vk, [byte][In]::MapVirtualKey($vk, 0), 0, [UIntPtr]::Zero); Start-Sleep -Milliseconds 30
        [In]::keybd_event([byte]$vk, [byte][In]::MapVirtualKey($vk, 0), 2, [UIntPtr]::Zero)
        [array]::Reverse($mods)
        foreach ($m in $mods) { $v = $vkOf[$m.ToLower()]; [In]::keybd_event([byte]$v, [byte][In]::MapVirtualKey($v, 0), 2, [UIntPtr]::Zero) }
    } elseif ($k -match '^SIZE (\d+),(\d+)$') {
        # The front window restored and sized w × h at the screen's corner.
        $h = [In]::GetForegroundWindow()
        [In]::ShowWindow($h, 9) | Out-Null; Start-Sleep -Milliseconds 300
        [In]::MoveWindow($h, 0, 0, [int]$Matches[1], [int]$Matches[2], $true) | Out-Null
    } elseif ($k -eq 'CLOSE') {
        # WM_CLOSE to the front window (the program's own close path; .NET's
        # main-window lookup misses some windows).
        [In]::PostMessage([In]::GetForegroundWindow(), 0x10, [IntPtr]::Zero, [IntPtr]::Zero) | Out-Null
    } elseif ($k -eq 'UP') {
        [In]::mouse_event(0x4, 0, 0, 0, [UIntPtr]::Zero)
    } elseif ($k -match '^WHEEL (\d+),(\d+),(-?\d+)$') {
        [In]::SetCursorPos([int]$Matches[1], [int]$Matches[2]) | Out-Null; Start-Sleep -Milliseconds 120
        $n = [int]$Matches[3]; $step = if ($n -gt 0) { 120 } else { -120 }
        for ($i = 0; $i -lt [Math]::Abs($n); $i++) { [In]::mouse_event(0x800, 0, 0, $step, [UIntPtr]::Zero); Start-Sleep -Milliseconds 80 }
    } elseif ($k -match '^CURSOR (.+)$') {
        $c = [In]::Cursor()
        $c | Out-File (Join-Path (Join-Path $here 'shots') $Matches[1]) -Append -Encoding utf8
        # Not a system cursor: its image too (shots/cursor_<handle>.png).
        if ($c -match '^([0-9A-F]+) at ') {
            Add-Type -AssemblyName System.Drawing
            $bmp = New-Object System.Drawing.Bitmap 32, 32
            $g = [System.Drawing.Graphics]::FromImage($bmp); $g.Clear([System.Drawing.Color]::White)
            $dc = $g.GetHdc(); [In]::DrawIconEx($dc, 0, 0, [IntPtr][Convert]::ToInt64($Matches[1], 16), 32, 32, 0, [IntPtr]::Zero, 3) | Out-Null; $g.ReleaseHdc($dc)
            $g.Dispose(); $bmp.Save((Join-Path (Join-Path $here 'shots') ("cursor_" + $Matches[1] + ".png"))); $bmp.Dispose()
        }
    } elseif ($k -match '^FLOATAT (\d+),(\d+)$') {
        $r = [In]::FloatAt([uint32]$p.Id, [int]$Matches[1], [int]$Matches[2])
        "$k -> $r" | Out-File $log -Append -Encoding utf8
    } elseif ($k -match '^FOCUS (.+)$') {
        "--- after step $($k)`n" + [In]::Focus() | Out-File (Join-Path (Join-Path $here 'shots') $Matches[1]) -Append -Encoding utf8
    } elseif ($k -match '^CLIPEMF (.+)$') {
        $b = [In]::ClipEmf()
        if ($b) { [IO.File]::WriteAllBytes((Join-Path (Join-Path $here 'shots') $Matches[1]), $b) } else { "no EMF" | Out-File $log -Append -Encoding utf8 }
    } elseif ($k -match '^CLIP (.+)$') {
        "--- after step $($k)`n" + [In]::Clip() | Out-File (Join-Path (Join-Path $here 'shots') $Matches[1]) -Append -Encoding utf8
    } elseif ($k -match '^TREE (.+)$') {
        [In]::Tree() | Out-File (Join-Path (Join-Path $here 'shots') $Matches[1]) -Encoding utf8
    } elseif ($k -match '^WINS (.+)$') {
        [In]::List([uint32]$p.Id) | Out-File (Join-Path (Join-Path $here 'shots') $Matches[1]) -Encoding utf8
    } elseif ($k -match '^SHOT (.+)$') {
        & (Join-Path $here 'shot.ps1') -Out (Join-Path (Join-Path $here 'shots') $Matches[1])
    } else {
        $wsh.SendKeys($k)
    }
    "step $k" | Out-File $log -Append -Encoding utf8
    Start-Sleep -Milliseconds 1500
}
if ($Shot) { & (Join-Path $here 'shot.ps1') -Out (Join-Path (Join-Path $here 'shots') $Shot) }
if (-not $Leave) { Stop-Process -Id $p.Id -Force -ErrorAction SilentlyContinue }
"done $(Get-Date -f s)" | Out-File $log -Append -Encoding utf8
