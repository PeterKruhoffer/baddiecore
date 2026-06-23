using Baddiecore.Features.Cms;
using Baddiecore.Models;
using Microsoft.AspNetCore.Mvc;

namespace Baddiecore.Controllers;

[ApiController]
[Route("api/content")]
public sealed class ContentController(ICmsQueries cmsQueries) : ControllerBase
{
    [HttpGet("tree")]
    public async Task<ActionResult<ContentTreeResponse>> GetTree(CancellationToken cancellationToken)
    {
        return Ok(await cmsQueries.GetContentTreeAsync(cancellationToken));
    }
}
