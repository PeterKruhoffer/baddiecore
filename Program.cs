using Baddiecore.Data;
using Baddiecore.Features.Cms;
using FluentValidation;
using Microsoft.EntityFrameworkCore;
using Microsoft.AspNetCore.StaticFiles;
using MySql.EntityFrameworkCore.Extensions;

var builder = WebApplication.CreateBuilder(args);

builder.Services.AddControllers();
builder.Services.AddOpenApi();
builder.Services.AddValidatorsFromAssemblyContaining<GetLayoutRequestValidator>();
builder.Services.AddDbContext<BaddiecoreDbContext>(options =>
{
    var connectionString = builder.Configuration.GetConnectionString("Baddiecore")
        ?? throw new InvalidOperationException("Connection string 'Baddiecore' is not configured.");

    options.UseMySQL(connectionString);
});
builder.Services.AddScoped<ICmsQueries, DatabaseCmsQueries>();
builder.Services.AddScoped<IRenderingDatasourceWriter, DatabaseRenderingDatasourceWriter>();

var app = builder.Build();

if (app.Environment.IsDevelopment())
{
    app.MapOpenApi();
}

app.UseHttpsRedirection();
app.UseDefaultFiles();
app.UseStaticFiles(new StaticFileOptions
{
    ContentTypeProvider = new FileExtensionContentTypeProvider(),
    OnPrepareResponse = context =>
    {
        if (context.Context.Request.Path.StartsWithSegments("/cms/assets"))
        {
            context.Context.Response.Headers["Access-Control-Allow-Origin"] = "*";
        }
    }
});

app.MapControllers();

app.MapFallbackToFile("/cms/{*path:nonfile}", "cms/index.html");

app.Run();

public partial class Program;
