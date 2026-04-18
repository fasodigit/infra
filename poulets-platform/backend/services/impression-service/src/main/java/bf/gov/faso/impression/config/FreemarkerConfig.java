package bf.gov.faso.impression.config;

import freemarker.template.Configuration;
import freemarker.template.TemplateExceptionHandler;
import org.springframework.context.annotation.Bean;
import org.springframework.context.annotation.Primary;

import java.io.File;
import java.io.IOException;
import java.nio.charset.StandardCharsets;

/**
 * Freemarker configuration for PDF template processing.
 */
@org.springframework.context.annotation.Configuration
public class FreemarkerConfig {

    @Bean
    @Primary
    public Configuration freemarkerConfiguration() throws IOException {
        Configuration cfg = new Configuration(Configuration.VERSION_2_3_32);

        // Try to load templates from classpath
        cfg.setClassLoaderForTemplateLoading(
            getClass().getClassLoader(),
            "templates"
        );

        // Settings
        cfg.setDefaultEncoding(StandardCharsets.UTF_8.name());
        cfg.setTemplateExceptionHandler(TemplateExceptionHandler.RETHROW_HANDLER);
        cfg.setLogTemplateExceptions(false);
        cfg.setWrapUncheckedExceptions(true);
        cfg.setFallbackOnNullLoopVariable(false);

        // Allow null values
        cfg.setClassicCompatible(true);

        return cfg;
    }
}
