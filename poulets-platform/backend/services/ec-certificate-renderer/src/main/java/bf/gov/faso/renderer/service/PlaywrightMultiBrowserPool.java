package bf.gov.faso.renderer.service;

import bf.gov.faso.renderer.config.RendererProperties;
import com.microsoft.playwright.*;
import jakarta.annotation.PostConstruct;
import jakarta.annotation.PreDestroy;
import org.slf4j.Logger;
import org.slf4j.LoggerFactory;
import org.springframework.stereotype.Component;

import java.nio.file.Path;
import java.util.ArrayList;
import java.util.List;
import java.util.concurrent.ArrayBlockingQueue;
import java.util.concurrent.BlockingQueue;
import java.util.concurrent.TimeUnit;
import java.util.concurrent.atomic.AtomicInteger;

@Component
public class PlaywrightMultiBrowserPool {

    private static final Logger log = LoggerFactory.getLogger(PlaywrightMultiBrowserPool.class);

    private final RendererProperties props;

    private int effectiveBrowserCount;
    private int pagesPerBrowser;
    private BlockingQueue<Page> pagePool;

    private final List<Playwright> playwrightInstances = new ArrayList<>();
    private final AtomicInteger inUseCount = new AtomicInteger(0);

    public PlaywrightMultiBrowserPool(RendererProperties props) {
        this.props = props;
    }

    @PostConstruct
    public void init() {
        this.effectiveBrowserCount = props.effectiveBrowserCount();
        this.pagesPerBrowser       = props.pagesPerBrowser();
        int totalSlots             = effectiveBrowserCount * pagesPerBrowser;
        this.pagePool              = new ArrayBlockingQueue<>(totalSlots);

        log.info("PlaywrightMultiBrowserPool — {} browsers x {} pages = {} slots",
                effectiveBrowserCount, pagesPerBrowser, totalSlots);

        for (int b = 0; b < effectiveBrowserCount; b++) {
            try {
                initBrowser(b);
            } catch (Exception e) {
                log.error("Failed to init browser #{} : {}", b, e.getMessage(), e);
                throw new IllegalStateException("Cannot start browser #" + b, e);
            }
        }

        log.info("Pool Playwright ready — {}/{} pages available", pagePool.size(), totalSlots);
    }

    private void initBrowser(int browserIndex) {
        log.info("  Starting browser #{} ...", browserIndex);

        Playwright pw = Playwright.create();
        playwrightInstances.add(pw);

        Browser browser = pw.chromium().launch(buildLaunchOptions());

        for (int p = 0; p < pagesPerBrowser; p++) {
            Page page = createWarmPage(browser, browserIndex, p);
            pagePool.offer(page);
        }

        log.info("  Browser #{} ready ({} pages warmed)", browserIndex, pagesPerBrowser);
    }

    private Page createWarmPage(Browser browser, int browserIdx, int pageIdx) {
        BrowserContext ctx = browser.newContext(buildContextOptions());
        Page page = ctx.newPage();

        page.route("**/*", route -> {
            String url = route.request().url();
            if (url.startsWith("data:") || url.equals("about:blank")
                    || url.startsWith("blob:")) {
                route.resume();
            } else {
                log.warn("External request blocked: {} (browser#{}/page#{})",
                        url, browserIdx, pageIdx);
                route.abort();
            }
        });

        page.navigate("about:blank");

        log.debug("    Page {}/{} created (browser#{})", pageIdx + 1, pagesPerBrowser, browserIdx);
        return page;
    }

    public Page acquire() throws InterruptedException {
        Page page = pagePool.poll(props.pageAcquireTimeoutMs(), TimeUnit.MILLISECONDS);
        if (page == null) {
            throw new IllegalStateException(
                    "Pool saturated: no page available after "
                    + props.pageAcquireTimeoutMs() + " ms. "
                    + "Active=" + inUseCount.get() + "/" + total());
        }
        inUseCount.incrementAndGet();
        return page;
    }

    public void release(Page page) {
        inUseCount.decrementAndGet();
        if (page == null) return;

        try {
            page.evaluate("() => { document.open(); document.close(); }");
            pagePool.offer(page);
        } catch (Exception e) {
            log.warn("Corrupted Playwright page — replacing: {}", e.getMessage());
            replacePage(page);
        }
    }

    private void replacePage(Page deadPage) {
        try {
            BrowserContext ctx = deadPage.context();
            Browser browser = ctx.browser();
            try { deadPage.close(); } catch (Exception ignored) {}

            Page fresh = createWarmPage(browser, -1, -1);
            pagePool.offer(fresh);
            log.info("Page replaced — pool: {}/{}", pagePool.size(), total());
        } catch (Exception e) {
            log.error("Cannot replace corrupted page: {}", e.getMessage(), e);
        }
    }

    public int available()     { return pagePool.size(); }
    public int total()         { return effectiveBrowserCount * pagesPerBrowser; }
    public int inUse()         { return inUseCount.get(); }

    public double saturation() {
        int t = total();
        if (t == 0) return 0.0;
        return (double) inUse() / t;
    }

    public boolean isHealthy() {
        return !playwrightInstances.isEmpty()
               && pagePool.size() > 0;
    }

    public int getBrowserCount()    { return effectiveBrowserCount; }
    public int getPagesPerBrowser() { return pagesPerBrowser; }

    @PreDestroy
    public void shutdown() {
        log.info("Shutting down PlaywrightMultiBrowserPool ({} instances)...",
                playwrightInstances.size());

        pagePool.clear();

        for (int i = 0; i < playwrightInstances.size(); i++) {
            Playwright pw = playwrightInstances.get(i);
            try {
                pw.close();
                log.debug("  Browser #{} closed", i);
            } catch (Exception e) {
                log.warn("  Error closing browser #{} : {}", i, e.getMessage());
            }
        }

        log.info("PlaywrightMultiBrowserPool stopped.");
    }

    private BrowserType.LaunchOptions buildLaunchOptions() {
        List<String> args = new ArrayList<>(List.of(
                "--no-sandbox",
                "--disable-setuid-sandbox",
                "--disable-dev-shm-usage",
                "--disable-gpu",
                "--disable-gpu-sandbox",
                "--disable-extensions",
                "--disable-background-networking",
                "--disable-background-timer-throttling",
                "--disable-backgrounding-occluded-windows",
                "--disable-default-apps",
                "--disable-sync",
                "--disable-translate",
                "--disable-features=TranslateUI,BlinkGenPropertyTrees",
                "--disable-ipc-flooding-protection",
                "--disable-renderer-backgrounding",
                "--no-first-run",
                "--no-default-browser-check",
                "--no-pings",
                "--font-render-hinting=none",
                "--force-color-profile=srgb",
                "--disable-web-security",
                "--allow-running-insecure-content",
                "--disable-partial-raster",
                "--enable-font-antialiasing",
                "--renderer-process-limit=" + pagesPerBrowser
        ));

        args.addAll(props.chromiumArgs());

        BrowserType.LaunchOptions opts = new BrowserType.LaunchOptions()
                .setHeadless(true)
                .setArgs(args);

        String chromiumPath = resolveChromiumPath(props.chromiumPath());
        if (!chromiumPath.isBlank()) {
            opts.setExecutablePath(Path.of(chromiumPath));
            log.debug("Chromium path: {}", chromiumPath);
        }

        return opts;
    }

    private Browser.NewContextOptions buildContextOptions() {
        return new Browser.NewContextOptions()
                .setLocale("fr-FR")
                .setTimezoneId("Africa/Ouagadougou")
                .setBypassCSP(true)
                .setJavaScriptEnabled(true);
    }

    private static String resolveChromiumPath(String configured) {
        if (configured != null && !configured.isBlank()) return configured;

        String fromEnv = System.getenv("CHROMIUM_PATH");
        if (fromEnv != null && !fromEnv.isBlank()) return fromEnv;

        String[] candidates = {
                "/usr/bin/chromium-browser",
                "/usr/bin/chromium",
                "/usr/bin/google-chrome-stable",
                "/usr/bin/google-chrome",
                "/snap/bin/chromium",
                "/Applications/Chromium.app/Contents/MacOS/Chromium",
                "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
        };
        for (String p : candidates) {
            if (Path.of(p).toFile().exists()) return p;
        }
        return "";
    }
}
