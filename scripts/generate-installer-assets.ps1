param(
  [string]$ProjectRoot = (Split-Path -Parent $PSScriptRoot)
)

$ErrorActionPreference = "Stop"

Add-Type -AssemblyName System.Drawing

$installerDirectory = Join-Path $ProjectRoot "src-tauri\installer"
$sourceDirectory = Join-Path $installerDirectory "source"
$logoPath = Join-Path $ProjectRoot "src-tauri\icons\icon.png"
$sidebarBackgroundPath = Join-Path $sourceDirectory "sidebar-background.png"
$bannerBackgroundPath = Join-Path $sourceDirectory "banner-background.png"

$requiredFiles = @($logoPath, $sidebarBackgroundPath, $bannerBackgroundPath)
foreach ($requiredFile in $requiredFiles) {
  if (-not (Test-Path -LiteralPath $requiredFile -PathType Leaf)) {
    throw "Missing installer artwork source: $requiredFile"
  }
}

New-Item -ItemType Directory -Force -Path $installerDirectory | Out-Null

function New-InstallerBitmap {
  param(
    [int]$Width,
    [int]$Height
  )

  $bitmap = [System.Drawing.Bitmap]::new(
    $Width,
    $Height,
    [System.Drawing.Imaging.PixelFormat]::Format24bppRgb
  )
  $bitmap.SetResolution(96, 96)
  return $bitmap
}

function Set-HighQualityRendering {
  param([System.Drawing.Graphics]$Graphics)

  $Graphics.CompositingMode = [System.Drawing.Drawing2D.CompositingMode]::SourceOver
  $Graphics.CompositingQuality = [System.Drawing.Drawing2D.CompositingQuality]::HighQuality
  $Graphics.InterpolationMode = [System.Drawing.Drawing2D.InterpolationMode]::HighQualityBicubic
  $Graphics.PixelOffsetMode = [System.Drawing.Drawing2D.PixelOffsetMode]::HighQuality
  $Graphics.SmoothingMode = [System.Drawing.Drawing2D.SmoothingMode]::HighQuality
  $Graphics.TextRenderingHint = [System.Drawing.Text.TextRenderingHint]::ClearTypeGridFit
}

function Draw-CoverImage {
  param(
    [System.Drawing.Graphics]$Graphics,
    [System.Drawing.Image]$Image,
    [System.Drawing.Rectangle]$Destination
  )

  $sourceRatio = $Image.Width / $Image.Height
  $destinationRatio = $Destination.Width / $Destination.Height

  if ($sourceRatio -gt $destinationRatio) {
    $sourceHeight = $Image.Height
    $sourceWidth = [int]($sourceHeight * $destinationRatio)
    $sourceX = [int](($Image.Width - $sourceWidth) / 2)
    $sourceY = 0
  } else {
    $sourceWidth = $Image.Width
    $sourceHeight = [int]($sourceWidth / $destinationRatio)
    $sourceX = 0
    $sourceY = [int](($Image.Height - $sourceHeight) / 2)
  }

  $source = [System.Drawing.Rectangle]::new($sourceX, $sourceY, $sourceWidth, $sourceHeight)
  $Graphics.DrawImage($Image, $Destination, $source, [System.Drawing.GraphicsUnit]::Pixel)
}

function Draw-Logo {
  param(
    [System.Drawing.Graphics]$Graphics,
    [System.Drawing.Image]$Logo,
    [int]$X,
    [int]$Y,
    [int]$Size
  )

  $destination = [System.Drawing.Rectangle]::new($X, $Y, $Size, $Size)
  $Graphics.DrawImage($Logo, $destination)
}

function Draw-LogoPlate {
  param(
    [System.Drawing.Graphics]$Graphics,
    [System.Drawing.Image]$Logo,
    [int]$X,
    [int]$Y,
    [int]$Size
  )

  $plateBrush = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::White)
  $platePen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(218, 218, 218), 1)
  try {
    $Graphics.FillEllipse($plateBrush, $X, $Y, $Size, $Size)
    $Graphics.DrawEllipse($platePen, $X, $Y, $Size - 1, $Size - 1)
    $logoInset = [int][Math]::Round($Size * 0.13)
    Draw-Logo -Graphics $Graphics -Logo $Logo -X ($X + $logoInset) -Y ($Y + $logoInset) -Size ($Size - (2 * $logoInset))
  } finally {
    $platePen.Dispose()
    $plateBrush.Dispose()
  }
}

function Draw-CenteredText {
  param(
    [System.Drawing.Graphics]$Graphics,
    [string]$Text,
    [System.Drawing.Font]$Font,
    [System.Drawing.Brush]$Brush,
    [System.Drawing.RectangleF]$Bounds
  )

  $format = [System.Drawing.StringFormat]::new()
  try {
    $format.Alignment = [System.Drawing.StringAlignment]::Center
    $format.LineAlignment = [System.Drawing.StringAlignment]::Center
    $Graphics.DrawString($Text, $Font, $Brush, $Bounds, $format)
  } finally {
    $format.Dispose()
  }
}

function Save-Bitmap {
  param(
    [System.Drawing.Bitmap]$Bitmap,
    [string]$FileName
  )

  $path = Join-Path $installerDirectory $FileName
  $Bitmap.Save($path, [System.Drawing.Imaging.ImageFormat]::Bmp)
  Write-Output "Generated $path ($($Bitmap.Width)x$($Bitmap.Height))"
}

$logo = [System.Drawing.Image]::FromFile($logoPath)
$sidebarBackground = [System.Drawing.Image]::FromFile($sidebarBackgroundPath)
$bannerBackground = [System.Drawing.Image]::FromFile($bannerBackgroundPath)
$brandBrush = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(15, 15, 15))
$mutedBrush = [System.Drawing.SolidBrush]::new([System.Drawing.Color]::FromArgb(96, 96, 96))
$productFont = [System.Drawing.Font]::new("Segoe UI", 18, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)
$taglineFont = [System.Drawing.Font]::new("Segoe UI", 7, [System.Drawing.FontStyle]::Bold, [System.Drawing.GraphicsUnit]::Pixel)

try {
  $nsisSidebar = New-InstallerBitmap -Width 164 -Height 314
  try {
    $graphics = [System.Drawing.Graphics]::FromImage($nsisSidebar)
    try {
      Set-HighQualityRendering -Graphics $graphics
      Draw-CoverImage -Graphics $graphics -Image $sidebarBackground -Destination ([System.Drawing.Rectangle]::new(0, 0, 164, 314))
      Draw-Logo -Graphics $graphics -Logo $logo -X 37 -Y 52 -Size 90
      Draw-CenteredText -Graphics $graphics -Text "SpringCat" -Font $productFont -Brush $brandBrush -Bounds ([System.Drawing.RectangleF]::new(8, 158, 148, 30))
      Draw-CenteredText -Graphics $graphics -Text "AI TASK COMPANION" -Font $taglineFont -Brush $mutedBrush -Bounds ([System.Drawing.RectangleF]::new(8, 188, 148, 18))
    } finally {
      $graphics.Dispose()
    }
    Save-Bitmap -Bitmap $nsisSidebar -FileName "nsis-sidebar.bmp"
  } finally {
    $nsisSidebar.Dispose()
  }

  $nsisHeader = New-InstallerBitmap -Width 150 -Height 57
  try {
    $graphics = [System.Drawing.Graphics]::FromImage($nsisHeader)
    try {
      Set-HighQualityRendering -Graphics $graphics
      Draw-CoverImage -Graphics $graphics -Image $bannerBackground -Destination ([System.Drawing.Rectangle]::new(0, 0, 150, 57))
      Draw-LogoPlate -Graphics $graphics -Logo $logo -X 101 -Y 5 -Size 47
    } finally {
      $graphics.Dispose()
    }
    Save-Bitmap -Bitmap $nsisHeader -FileName "nsis-header.bmp"
  } finally {
    $nsisHeader.Dispose()
  }

  $wixBanner = New-InstallerBitmap -Width 493 -Height 58
  try {
    $graphics = [System.Drawing.Graphics]::FromImage($wixBanner)
    try {
      Set-HighQualityRendering -Graphics $graphics
      Draw-CoverImage -Graphics $graphics -Image $bannerBackground -Destination ([System.Drawing.Rectangle]::new(0, 0, 493, 58))
      Draw-LogoPlate -Graphics $graphics -Logo $logo -X 440 -Y 4 -Size 49
    } finally {
      $graphics.Dispose()
    }
    Save-Bitmap -Bitmap $wixBanner -FileName "wix-banner.bmp"
  } finally {
    $wixBanner.Dispose()
  }

  $wixDialog = New-InstallerBitmap -Width 493 -Height 312
  try {
    $graphics = [System.Drawing.Graphics]::FromImage($wixDialog)
    try {
      Set-HighQualityRendering -Graphics $graphics
      $graphics.Clear([System.Drawing.Color]::FromArgb(250, 249, 247))
      Draw-CoverImage -Graphics $graphics -Image $sidebarBackground -Destination ([System.Drawing.Rectangle]::new(0, 0, 164, 312))
      $separatorPen = [System.Drawing.Pen]::new([System.Drawing.Color]::FromArgb(225, 225, 225), 1)
      try {
        $graphics.DrawLine($separatorPen, 163, 0, 163, 312)
      } finally {
        $separatorPen.Dispose()
      }
      Draw-Logo -Graphics $graphics -Logo $logo -X 42 -Y 58 -Size 82
      Draw-CenteredText -Graphics $graphics -Text "SpringCat" -Font $productFont -Brush $brandBrush -Bounds ([System.Drawing.RectangleF]::new(12, 151, 142, 30))
      Draw-CenteredText -Graphics $graphics -Text "AI TASK COMPANION" -Font $taglineFont -Brush $mutedBrush -Bounds ([System.Drawing.RectangleF]::new(12, 181, 142, 18))
    } finally {
      $graphics.Dispose()
    }
    Save-Bitmap -Bitmap $wixDialog -FileName "wix-dialog.bmp"
  } finally {
    $wixDialog.Dispose()
  }
} finally {
  $taglineFont.Dispose()
  $productFont.Dispose()
  $mutedBrush.Dispose()
  $brandBrush.Dispose()
  $bannerBackground.Dispose()
  $sidebarBackground.Dispose()
  $logo.Dispose()
}
