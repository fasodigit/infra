// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import {
  AfterViewInit, Directive, ElementRef, Input, OnDestroy, PLATFORM_ID, inject,
} from '@angular/core';
import { isPlatformBrowser } from '@angular/common';

/**
 * Ajoute la classe `.is-visible` quand l'élément entre dans le viewport.
 * Les animations CSS doivent cibler `.is-visible` pour se déclencher.
 *
 * Usage :
 *   <section appAnimateOnScroll>…</section>
 *   <section appAnimateOnScroll delay="100">…</section>
 *
 * Styles attendus :
 *   :host { opacity: 0; transform: translateY(16px); transition: opacity 600ms, transform 600ms; }
 *   :host.is-visible { opacity: 1; transform: none; }
 *
 * Respecte `prefers-reduced-motion` : applique immédiatement la classe sans transition.
 */
@Directive({
  selector: '[appAnimateOnScroll]',
  standalone: true,
})
export class AnimateOnScrollDirective implements AfterViewInit, OnDestroy {
  private readonly el = inject(ElementRef<HTMLElement>);
  private readonly platformId = inject(PLATFORM_ID);

  /** Seuil de déclenchement 0-1 (0.15 = 15% visible). Défaut 0.12. */
  @Input() threshold = 0.12;
  /** Délai ms avant application. Utile pour staggered animations. */
  @Input() delay = 0;
  /** Si true, se redéclenche quand on scrolle out puis in. */
  @Input() once = true;

  private observer?: IntersectionObserver;

  ngAfterViewInit(): void {
    if (!isPlatformBrowser(this.platformId)) return;

    const node = this.el.nativeElement;

    // prefers-reduced-motion : pas d'animation, juste révélation immédiate
    const reduce = typeof matchMedia !== 'undefined'
      && matchMedia('(prefers-reduced-motion: reduce)').matches;

    if (reduce) {
      node.classList.add('is-visible');
      return;
    }

    if (typeof IntersectionObserver === 'undefined') {
      node.classList.add('is-visible');
      return;
    }

    this.observer = new IntersectionObserver((entries) => {
      for (const entry of entries) {
        if (entry.isIntersecting) {
          if (this.delay > 0) {
            setTimeout(() => node.classList.add('is-visible'), this.delay);
          } else {
            node.classList.add('is-visible');
          }
          if (this.once) this.observer?.unobserve(node);
        } else if (!this.once) {
          node.classList.remove('is-visible');
        }
      }
    }, { threshold: this.threshold, rootMargin: '0px 0px -5% 0px' });

    this.observer.observe(node);
  }

  ngOnDestroy(): void {
    this.observer?.disconnect();
  }
}
