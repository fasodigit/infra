package bf.gov.faso.renderer;

import bf.gov.faso.renderer.config.RendererProperties;
import org.springframework.boot.SpringApplication;
import org.springframework.boot.autoconfigure.SpringBootApplication;
import org.springframework.boot.autoconfigure.jdbc.DataSourceAutoConfiguration;
import org.springframework.boot.autoconfigure.orm.jpa.HibernateJpaAutoConfiguration;
import org.springframework.boot.context.properties.EnableConfigurationProperties;
import org.springframework.context.annotation.Bean;
import reactor.core.scheduler.Scheduler;
import reactor.core.scheduler.Schedulers;

import java.util.concurrent.Executors;

@SpringBootApplication(exclude = {
    DataSourceAutoConfiguration.class,
    HibernateJpaAutoConfiguration.class
})
@EnableConfigurationProperties(RendererProperties.class)
public class EcCertificateRendererApplication {

    public static void main(String[] args) {
        SpringApplication.run(EcCertificateRendererApplication.class, args);
    }

    /**
     * Scheduler réactif basé sur Virtual Threads.
     * Les appels bloquants Playwright sont délégués ici pour ne pas bloquer l'event-loop Netty.
     */
    @Bean("vtScheduler")
    public Scheduler vtScheduler() {
        var factory = Thread.ofVirtual().name("vt-renderer-", 0).factory();
        return Schedulers.fromExecutor(Executors.newThreadPerTaskExecutor(factory));
    }
}
