// SPDX-License-Identifier: AGPL-3.0-or-later
// Copyright (C) 2026 FASO DIGITALISATION

import {
  Directive,
  Input,
  TemplateRef,
  ViewContainerRef,
  OnDestroy,
  OnInit,
  inject,
} from '@angular/core';
import { Subscription } from 'rxjs';
import { FeatureFlagsService } from './feature-flags.service';

/**
 * Directive structurelle `*fasoFeature`.
 *
 * Usage :
 * ```html
 * <section *fasoFeature="'poulets.new-checkout'">
 *   <app-new-checkout />
 * </section>
 *
 * <section *fasoFeature="'poulets.new-checkout'; else oldFlow">
 *   <app-new-checkout />
 * </section>
 * <ng-template #oldFlow><app-old-checkout /></ng-template>
 * ```
 *
 * Réagit dynamiquement à {@link FeatureFlagsService.flags$} : si le flag est
 * basculé côté GrowthBook / BFF, la vue est rendue / détruite sans reload.
 */
@Directive({
  selector: '[fasoFeature]',
  standalone: true,
})
export class FasoFeatureDirective implements OnInit, OnDestroy {
  private readonly tpl = inject(TemplateRef<unknown>);
  private readonly vcr = inject(ViewContainerRef);
  private readonly flags = inject(FeatureFlagsService);
  private sub?: Subscription;

  private flagKey = '';
  private elseTpl: TemplateRef<unknown> | null = null;
  private lastShown: 'then' | 'else' | 'none' = 'none';

  @Input({ required: true }) set fasoFeature(value: string) {
    this.flagKey = value;
    this.render();
  }

  @Input() set fasoFeatureElse(ref: TemplateRef<unknown> | null) {
    this.elseTpl = ref;
    this.render();
  }

  ngOnInit(): void {
    this.sub = this.flags.flags$.subscribe(() => this.render());
  }

  ngOnDestroy(): void {
    this.sub?.unsubscribe();
  }

  private render(): void {
    const on = this.flags.isOn(this.flagKey);
    const want: 'then' | 'else' | 'none' = on ? 'then' : this.elseTpl ? 'else' : 'none';
    if (want === this.lastShown) return;

    this.vcr.clear();
    if (want === 'then') {
      this.vcr.createEmbeddedView(this.tpl);
    } else if (want === 'else' && this.elseTpl) {
      this.vcr.createEmbeddedView(this.elseTpl);
    }
    this.lastShown = want;
  }
}
