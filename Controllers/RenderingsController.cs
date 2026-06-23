using Baddiecore.Features.Cms;
using Baddiecore.Models;
using Microsoft.AspNetCore.Mvc;

namespace Baddiecore.Controllers;

[ApiController]
[Route("api/renderings")]
public sealed class RenderingsController(ICmsQueries cmsQueries) : ControllerBase
{
    [HttpGet]
    public async Task<ActionResult<RenderingCatalogResponse>> GetRenderings(CancellationToken cancellationToken)
    {
        return Ok(await cmsQueries.GetRenderingsAsync(cancellationToken));
    }
}
