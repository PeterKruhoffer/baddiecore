using Baddiecore.Features.Cms;
using Baddiecore.Models;
using Microsoft.AspNetCore.Mvc;

namespace Baddiecore.Controllers;

[ApiController]
[Route("api/layout")]
public sealed class LayoutController(ICmsQueries cmsQueries) : ControllerBase
{
    [HttpGet]
    public async Task<ActionResult<LayoutResponse>> GetLayout(
        [FromQuery] string path = "/",
        [FromQuery] string mode = "preview",
        CancellationToken cancellationToken = default)
    {
        if (!CmsLayoutModes.IsKnown(mode))
        {
            return BadRequest(new
            {
                Message = "Layout mode must be either 'preview' or 'published'."
            });
        }

        var layout = await cmsQueries.GetLayoutAsync(path, mode, cancellationToken);

        if (layout is null)
        {
            return NotFound(new
            {
                Message = $"No layout exists for path '{path}'."
            });
        }

        return Ok(layout);
    }
}
