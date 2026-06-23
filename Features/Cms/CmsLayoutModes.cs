namespace Baddiecore.Features.Cms;

public static class CmsLayoutModes
{
    public const string Preview = "preview";
    public const string Published = "published";

    public static string Normalize(string? mode)
    {
        return string.IsNullOrWhiteSpace(mode)
            ? Preview
            : mode.Trim().ToLowerInvariant();
    }

    public static bool IsKnown(string? mode)
    {
        var normalizedMode = Normalize(mode);
        return normalizedMode is Preview or Published;
    }
}
