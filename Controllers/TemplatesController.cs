using Baddiecore.Features.Cms;
using Baddiecore.Models;
using Microsoft.AspNetCore.Mvc;

namespace Baddiecore.Controllers;

[ApiController]
[Route("api/templates")]
public sealed class TemplatesController(ICmsQueries cmsQueries) : ControllerBase
{
    [HttpGet]
    public async Task<ActionResult<TemplateCatalogResponse>> GetTemplates(CancellationToken cancellationToken)
    {
        return Ok(await cmsQueries.GetTemplatesAsync(cancellationToken));
    }
}
