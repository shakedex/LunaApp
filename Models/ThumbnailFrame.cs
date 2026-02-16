using Avalonia.Media.Imaging;

namespace LunaApp.Models;

/// <summary>
/// Represents a single thumbnail frame extracted from a clip.
/// </summary>
public sealed class ThumbnailFrame
{
    /// <summary>
    /// Position in the clip (0.0 = start, 0.5 = middle, 1.0 = end)
    /// </summary>
    public double Position { get; init; }
    
    /// <summary>
    /// Timecode at this frame position
    /// </summary>
    public string? Timecode { get; set; }
    
    /// <summary>
    /// Frame number in the clip
    /// </summary>
    public int FrameNumber { get; set; }
    
    /// <summary>
    /// Base64 encoded image data (WebP or JPEG)
    /// </summary>
    public string? ImageBase64 { get; set; }
    
    /// <summary>
    /// Path to thumbnail file on disk (if saved externally)
    /// </summary>
    public string? ImagePath { get; set; }
    
    /// <summary>
    /// Image width in pixels
    /// </summary>
    public int Width { get; set; }
    
    /// <summary>
    /// Image height in pixels
    /// </summary>
    public int Height { get; set; }
    
    /// <summary>
    /// Gets the image source for display (data URI or file path)
    /// </summary>
    public string ImageSource => !string.IsNullOrEmpty(ImageBase64) 
        ? $"data:image/webp;base64,{ImageBase64}" 
        : ImagePath ?? string.Empty;
    
    /// <summary>
    /// Cached bitmap for UI display
    /// </summary>
    private Bitmap? _bitmap;
    
    /// <summary>
    /// Gets a Bitmap for Avalonia UI display. Lazily loads from Base64 or file path.
    /// </summary>
    public Bitmap? Bitmap
    {
        get
        {
            if (_bitmap != null)
                return _bitmap;
            
            try
            {
                if (!string.IsNullOrEmpty(ImageBase64))
                {
                    var bytes = Convert.FromBase64String(ImageBase64);
                    using var stream = new MemoryStream(bytes);
                    _bitmap = new Bitmap(stream);
                }
                else if (!string.IsNullOrEmpty(ImagePath) && File.Exists(ImagePath))
                {
                    _bitmap = new Bitmap(ImagePath);
                }
            }
            catch
            {
                // Failed to load bitmap - return null
            }
            
            return _bitmap;
        }
    }
    
    /// <summary>
    /// Whether this thumbnail has a valid image
    /// </summary>
    public bool HasImage => !string.IsNullOrEmpty(ImageBase64) || (!string.IsNullOrEmpty(ImagePath) && File.Exists(ImagePath));
}
