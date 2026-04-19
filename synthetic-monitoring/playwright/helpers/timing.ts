// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION
//
// Performance timing helpers — wraps the Navigation Timing API, Paint Timing
// API and LargestContentfulPaint observer to collect FCP / LCP / TTI inside
// the browser context and return them to the Node test runner.

import type { Page, Response } from '@playwright/test';

export interface PerfMetrics {
  /** Navigation start → loadEventEnd, ms. */
  totalDurationMs: number;
  /** First Contentful Paint, ms since navigationStart. */
  firstContentfulPaintMs: number;
  /** Largest Contentful Paint, ms. */
  largestContentfulPaintMs: number;
  /** Time To Interactive (domInteractive as proxy), ms. */
  timeToInteractiveMs: number;
  /** HTTP 5xx responses observed on this page. */
  http5xxCount: number;
}

/**
 * Attach a response listener that counts HTTP 5xx responses.
 * Must be called before `page.goto()`.
 */
export function trackHttp5xx(page: Page): { count: () => number } {
  let count = 0;
  const handler = (response: Response) => {
    if (response.status() >= 500 && response.status() < 600) {
      count += 1;
    }
  };
  page.on('response', handler);
  return { count: () => count };
}

/**
 * Collect page performance metrics after the page has loaded.
 * Uses `performance.timing` + Paint Timing API + LargestContentfulPaint
 * PerformanceObserver (polled up to 5 s).
 */
export async function collectPerfMetrics(
  page: Page,
  http5xxCount: number,
): Promise<PerfMetrics> {
  const raw = await page.evaluate(async () => {
    const paintEntries = performance.getEntriesByType('paint');
    const fcpEntry = paintEntries.find((e) => e.name === 'first-contentful-paint');
    const firstContentfulPaintMs = fcpEntry ? fcpEntry.startTime : 0;

    const nav = performance.getEntriesByType('navigation')[0] as
      | PerformanceNavigationTiming
      | undefined;
    const totalDurationMs = nav ? nav.loadEventEnd - nav.startTime : 0;
    const timeToInteractiveMs = nav ? nav.domInteractive - nav.startTime : 0;

    // LCP: observe up to 5 s (or resolve on paint quiescence).
    const largestContentfulPaintMs = await new Promise<number>((resolve) => {
      let last = 0;
      try {
        const obs = new PerformanceObserver((list) => {
          for (const entry of list.getEntries()) {
            last = Math.max(last, entry.startTime);
          }
        });
        obs.observe({ type: 'largest-contentful-paint', buffered: true });
        setTimeout(() => {
          obs.disconnect();
          resolve(last);
        }, 5000);
      } catch {
        resolve(0);
      }
    });

    return {
      totalDurationMs,
      firstContentfulPaintMs,
      largestContentfulPaintMs,
      timeToInteractiveMs,
    };
  });

  return { ...raw, http5xxCount };
}

/**
 * Measure duration of an async step in milliseconds.
 */
export async function measureStep<T>(
  label: string,
  fn: () => Promise<T>,
): Promise<{ label: string; durationMs: number; result: T }> {
  const start = Date.now();
  const result = await fn();
  return { label, durationMs: Date.now() - start, result };
}
