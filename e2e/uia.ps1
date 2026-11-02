# Drives native Windows dialogs (the NSIS uninstaller and its message boxes)
# for the installer tests. Windows are found with UI Automation; buttons are
# pressed with window messages, because UI Automation shows the uninstaller's
# Win32 controls only as plain panes.
#
#   uia.ps1 -Action click   -Window "CODE X MART Uninstall" -Name "Next >"
#   uia.ps1 -Action select  -Window "CODE X MART Uninstall" -Name "Delete all my data and credentials"
#   uia.ps1 -Action answer  -Window "CODE X MART Uninstall" -Text "Are you sure" -Name "No"
#   uia.ps1 -Action wait    -Window "CODE X MART Uninstall" -Text "Your CODE X MART data"
#   uia.ps1 -Action gone    -Window "CODE X MART Uninstall"
#   uia.ps1 -Action shot    -Window "CODE X MART Uninstall" -Name C:\path\file.png
#   uia.ps1 -Action dump    -Window "CODE X MART Uninstall"
#
# Prints OK (and the window text for wait/answer) or exits with code 1.
param(
  [Parameter(Mandatory = $true)][string]$Action,
  [Parameter(Mandatory = $true)][string]$Window,
  [string]$Name = '',
  [string]$Text = '',
  [int]$Timeout = 60
)
$ErrorActionPreference = 'Stop'
Add-Type -AssemblyName UIAutomationClient
Add-Type -AssemblyName UIAutomationTypes
Add-Type @'
using System;
using System.Runtime.InteropServices;
public static class CxmWin32 {
  [DllImport("user32.dll")] public static extern IntPtr GetParent(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern int GetDlgCtrlID(IntPtr hWnd);
  [DllImport("user32.dll")] public static extern bool PostMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
  [DllImport("user32.dll")] public static extern IntPtr SendMessage(IntPtr hWnd, uint msg, IntPtr wParam, IntPtr lParam);
  [DllImport("user32.dll")] public static extern bool IsWindowEnabled(IntPtr hWnd);
  // Presses a button the way a click does: WM_COMMAND(BN_CLICKED) to its
  // parent. Posted, so a message box opened by the click cannot block us.
  public static void Press(IntPtr button) {
    int id = GetDlgCtrlID(button);
    PostMessage(GetParent(button), 0x0111, (IntPtr)(id & 0xFFFF), button);
  }
  // Selects a radio button (BM_CLICK updates its check state).
  public static void Select(IntPtr radio) { SendMessage(radio, 0x00F5, IntPtr.Zero, IntPtr.Zero); }
}
'@
$root = [System.Windows.Automation.AutomationElement]::RootElement
$deadline = (Get-Date).AddSeconds($Timeout)

# Top-level windows whose title starts with $title: NSIS adds the page to the
# title (": Confirmation", ": Uninstalling", ": Completed").
function Windows-Named($title) {
  return $root.FindAll([System.Windows.Automation.TreeScope]::Children, [System.Windows.Automation.Condition]::TrueCondition) |
    Where-Object { $_.Current.Name -like "$title*" }
}

function All-Text($el) {
  $texts = $el.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) |
    ForEach-Object { $_.Current.Name } | Where-Object { $_ }
  return ($texts -join "`n")
}

# A window with the title that contains $Text (any window if $Text is empty).
function Find-Window {
  foreach ($w in (Windows-Named $Window)) {
    if (-not $Text) { return $w }
    if ((All-Text $w) -like "*$Text*") { return $w }
  }
  return $null
}

function Find-Control($win, $label) {
  $cond = New-Object System.Windows.Automation.PropertyCondition([System.Windows.Automation.AutomationElement]::NameProperty, $label)
  return $win.FindFirst([System.Windows.Automation.TreeScope]::Descendants, $cond)
}

if ($Action -eq 'dump') {
  foreach ($w in (Windows-Named $Window)) {
    "WINDOW pid=$($w.Current.ProcessId) class=$($w.Current.ClassName)"
    $w.FindAll([System.Windows.Automation.TreeScope]::Descendants, [System.Windows.Automation.Condition]::TrueCondition) | ForEach-Object {
      "  $($_.Current.ControlType.ProgrammaticName) name='$($_.Current.Name)' enabled=$($_.Current.IsEnabled) hwnd=$($_.Current.NativeWindowHandle)"
    }
  }
  exit 0
}

while ((Get-Date) -lt $deadline) {
  try {
    $win = Find-Window
    if ($Action -eq 'gone') {
      if (-not $win) { 'OK'; exit 0 }
      Start-Sleep -Milliseconds 300
      continue
    }
    if ($win) {
      switch ($Action) {
        'wait' { 'OK'; All-Text $win; exit 0 }
        { $_ -in 'click', 'answer' } {
          $ctl = Find-Control $win $Name
          if ($ctl) {
            $hwnd = [IntPtr]$ctl.Current.NativeWindowHandle
            if ([CxmWin32]::IsWindowEnabled($hwnd)) {
              $winText = All-Text $win
              [CxmWin32]::Press($hwnd)
              'OK'
              if ($Action -eq 'answer') { $winText }
              exit 0
            }
          }
        }
        'select' {
          $ctl = Find-Control $win $Name
          if ($ctl) {
            [CxmWin32]::Select([IntPtr]$ctl.Current.NativeWindowHandle)
            'OK'
            exit 0
          }
        }
        'shot' {
          Add-Type -AssemblyName System.Drawing
          $r = $win.Current.BoundingRectangle
          $bmp = New-Object System.Drawing.Bitmap([int]$r.Width, [int]$r.Height)
          $g = [System.Drawing.Graphics]::FromImage($bmp)
          $g.CopyFromScreen([int]$r.X, [int]$r.Y, 0, 0, $bmp.Size)
          $bmp.Save($Name, [System.Drawing.Imaging.ImageFormat]::Png)
          'OK'
          exit 0
        }
        default { "Unknown action $Action"; exit 2 }
      }
    }
  } catch {
    # The window can close or change while it is being read; try again.
  }
  Start-Sleep -Milliseconds 300
}
"TIMEOUT: $Action '$Name' in window '$Window'$(if ($Text) { " containing '$Text'" })"
exit 1
