package bf.gov.faso.renderer.service;

import com.github.jknack.handlebars.Handlebars;
import com.github.jknack.handlebars.Helper;
import com.github.jknack.handlebars.Template;
import com.github.jknack.handlebars.io.ClassPathTemplateLoader;
import com.github.jknack.handlebars.io.FileTemplateLoader;
import jakarta.annotation.PostConstruct;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Service;

import java.io.IOException;
import java.nio.file.Files;
import java.nio.file.Path;
import java.text.NumberFormat;
import java.time.LocalDate;
import java.time.format.DateTimeFormatter;
import java.time.format.FormatStyle;
import java.util.*;
import java.util.concurrent.ConcurrentHashMap;

@Service
public class TemplateService {

    private static final Logger log = LoggerFactory.getLogger(TemplateService.class);

    private static final Locale LOCALE_FR = Locale.of("fr", "FR");
    private static final DateTimeFormatter FMT_SHORT =
            DateTimeFormatter.ofPattern("dd/MM/yyyy", LOCALE_FR);
    private static final DateTimeFormatter FMT_LONG =
            DateTimeFormatter.ofLocalizedDate(FormatStyle.LONG).withLocale(LOCALE_FR);

    /** Templates connus — includes ACTE_MARIAGE (missing from original docs). */
    private static final List<String> KNOWN_TEMPLATES = List.of(
            "ACTE_NAISSANCE",
            "ACTE_MARIAGE",
            "ACTE_DECES",
            "PERMIS_PORT_ARMES",
            "ACTE_DIVERS"
    );

    private final Map<String, Template> compiledTemplates = new ConcurrentHashMap<>();

    private final Handlebars handlebars;
    private final AssetInliner assetInliner;

    public TemplateService(AssetInliner assetInliner) {
        this.assetInliner = assetInliner;

        this.handlebars = new Handlebars(new FileTemplateLoader("./templates", ".hbs"))
                .with(new ClassPathTemplateLoader("/templates", ".hbs"));
        this.handlebars.setInfiniteLoops(false);
        this.handlebars.setPrettyPrint(false);
    }

    @PostConstruct
    public void precompileAll() {
        registerHelpers();

        log.info("=== TemplateService : pre-compiling templates ===");
        long start = System.currentTimeMillis();

        for (String name : KNOWN_TEMPLATES) {
            try {
                Template t = handlebars.compile(name);
                compiledTemplates.put(name, t);
                log.info("  Template compiled: {}", name);
            } catch (IOException e) {
                log.warn("  Template not found: {} ({})", name, e.getMessage());
            }
        }

        // Scan classpath for additional templates
        Path fsTemplates = Path.of("./templates");
        if (Files.isDirectory(fsTemplates)) {
            try (var paths = Files.walk(fsTemplates, 1)) {
                paths.filter(p -> p.toString().endsWith(".hbs"))
                     .filter(p -> !p.getFileName().toString().startsWith("_"))
                     .forEach(p -> {
                         String name = p.getFileName().toString().replace(".hbs", "");
                         if (!compiledTemplates.containsKey(name)) {
                             try {
                                 compiledTemplates.put(name, handlebars.compile(name));
                                 log.info("  Additional template compiled: {}", name);
                             } catch (IOException e) {
                                 log.error("  Error compiling {} : {}", name, e.getMessage());
                             }
                         }
                     });
            } catch (IOException e) {
                log.warn("Cannot scan ./templates: {}", e.getMessage());
            }
        }

        log.info("=== {} template(s) compiled in {} ms ===",
                compiledTemplates.size(), System.currentTimeMillis() - start);
    }

    public String render(String templateName, Map<String, Object> data) throws IOException {
        Objects.requireNonNull(templateName, "templateName cannot be null");

        Template tmpl = compiledTemplates.computeIfAbsent(templateName, name -> {
            try {
                Template t = handlebars.compile(name);
                log.info("Template compiled on-the-fly: {}", name);
                return t;
            } catch (IOException e) {
                throw new IllegalArgumentException("Unknown template: " + name, e);
            }
        });

        Map<String, Object> enrichedData = new HashMap<>(data);
        assetInliner.getAssetsForTemplate(templateName)
                    .forEach(enrichedData::putIfAbsent);

        return tmpl.apply(enrichedData);
    }

    public boolean hasTemplate(String name) {
        return compiledTemplates.containsKey(name);
    }

    public Set<String> availableTemplates() {
        return Set.copyOf(compiledTemplates.keySet());
    }

    private void registerHelpers() {

        handlebars.registerHelper("formatDate", (Helper<Object>) (ctx, options) -> {
            if (ctx == null) return "";
            try { return LocalDate.parse(ctx.toString()).format(FMT_SHORT); }
            catch (Exception e) { return ctx.toString(); }
        });

        handlebars.registerHelper("formatDateLong", (Helper<Object>) (ctx, options) -> {
            if (ctx == null) return "";
            try { return LocalDate.parse(ctx.toString()).format(FMT_LONG); }
            catch (Exception e) { return ctx.toString(); }
        });

        handlebars.registerHelper("formatCurrency", (Helper<Object>) (amount, options) -> {
            if (amount == null) return "";
            String currency = options.params.length > 0
                    ? options.param(0).toString() : "XOF";
            NumberFormat nf = NumberFormat.getNumberInstance(LOCALE_FR);
            nf.setMaximumFractionDigits(2);
            nf.setMinimumFractionDigits(0);
            return nf.format(Double.parseDouble(amount.toString())) + " " + currency;
        });

        handlebars.registerHelper("uppercase", (Helper<Object>) (ctx, options) ->
                ctx == null ? "" : ctx.toString().toUpperCase(LOCALE_FR));

        handlebars.registerHelper("lowercase", (Helper<Object>) (ctx, options) ->
                ctx == null ? "" : ctx.toString().toLowerCase(LOCALE_FR));

        handlebars.registerHelper("ifEquals", (Helper<Object>) (a, options) -> {
            Object b = options.param(0);
            if (a != null && a.equals(b)) return options.fn();
            return options.inverse();
        });

        handlebars.registerHelper("ifGender", (Helper<Object>) (sexe, options) -> {
            String male   = options.params.length > 0 ? options.param(0).toString() : "M.";
            String female = options.params.length > 1 ? options.param(1).toString() : "Mme";
            return "F".equalsIgnoreCase(sexe != null ? sexe.toString() : "") ? female : male;
        });

        handlebars.registerHelper("padLeft", (Helper<Object>) (value, options) -> {
            if (value == null) return "";
            int width = options.params.length > 0
                    ? Integer.parseInt(options.param(0).toString()) : 6;
            String pad = options.params.length > 1
                    ? options.param(1).toString() : "0";
            String s = value.toString();
            return pad.repeat(Math.max(0, width - s.length())) + s;
        });

        handlebars.registerHelper("formatNumeroActe", (Helper<Object>) (annee, options) -> {
            String numero = options.params.length > 0 ? options.param(0).toString() : "";
            return "N\u00B0" + (annee != null ? annee.toString() : "") + "/" + numero;
        });

        log.debug("Handlebars helpers registered (9 helpers)");
    }
}
