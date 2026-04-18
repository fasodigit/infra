package bf.gov.faso.renderer.util;

import bf.gov.faso.renderer.config.RendererProperties;
import bf.gov.faso.renderer.service.PlaywrightMultiBrowserPool;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.context.annotation.Lazy;
import org.springframework.stereotype.Component;

import java.util.concurrent.Semaphore;
import java.util.concurrent.atomic.AtomicInteger;

@Component
public class RenderSemaphore {

    private static final Logger log = LoggerFactory.getLogger(RenderSemaphore.class);

    private final RendererProperties props;
    private final PlaywrightMultiBrowserPool browserPool;

    private volatile Semaphore semaphore;
    private volatile int maxConcurrent;
    private final AtomicInteger activeTasks = new AtomicInteger(0);

    public RenderSemaphore(
            RendererProperties props,
            @Lazy PlaywrightMultiBrowserPool browserPool) {
        this.props       = props;
        this.browserPool = browserPool;
    }

    private Semaphore getSemaphore() {
        if (semaphore == null) {
            synchronized (this) {
                if (semaphore == null) {
                    this.maxConcurrent = props.effectiveMaxConcurrent(
                            browserPool.getBrowserCount());
                    this.semaphore = new Semaphore(maxConcurrent, true);
                    log.info("RenderSemaphore initialized — maxConcurrent={}", maxConcurrent);
                }
            }
        }
        return semaphore;
    }

    public boolean tryAcquire() {
        boolean acquired = getSemaphore().tryAcquire();
        if (acquired) {
            activeTasks.incrementAndGet();
        }
        return acquired;
    }

    public void release() {
        getSemaphore().release();
        activeTasks.decrementAndGet();
    }

    public int getActiveTasks()     { return activeTasks.get(); }
    public int getMaxConcurrent()   { return maxConcurrent; }
    public int getAvailableSlots()  { return getSemaphore().availablePermits(); }
}
