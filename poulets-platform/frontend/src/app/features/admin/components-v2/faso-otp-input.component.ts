// SPDX-License-Identifier: AGPL-3.0-or-later
// © 2026 FASO DIGITALISATION — Burkina Faso

import {
  AfterViewInit,
  ChangeDetectionStrategy,
  Component,
  ElementRef,
  computed,
  effect,
  input,
  model,
  output,
  viewChildren,
} from '@angular/core';
import { CommonModule } from '@angular/common';

/**
 * Entrée segmentée OTP (6 à 10 chiffres). Gère :
 *  - auto-focus suivant après saisie ;
 *  - retour arrière via `Backspace` (vide la case courante puis recule) ;
 *  - collage (`paste`) : étale les chiffres sur les cases ;
 *  - flèches gauche/droite ;
 *  - émission `complete` quand toutes les cases sont remplies.
 */
@Component({
  selector: 'faso-otp-input',
  standalone: true,
  imports: [CommonModule],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <div class="fd-otp">
      @for (slot of slots(); track $index; let i = $index) {
        <input
          #cell
          type="text"
          inputmode="numeric"
          autocomplete="one-time-code"
          maxlength="1"
          [attr.aria-label]="'OTP digit ' + (i + 1)"
          [disabled]="disabled()"
          [value]="slot"
          (input)="onInput($event, i)"
          (keydown)="onKeydown($event, i)"
          (paste)="onPaste($event, i)"
          (focus)="onFocus($event)"
        />
      }
    </div>
  `,
})
export class FasoOtpInputComponent implements AfterViewInit {
  readonly length = input<number>(8);
  readonly value = model<string>('');
  readonly disabled = input<boolean>(false);
  readonly complete = output<string>();

  private readonly cells =
    viewChildren<ElementRef<HTMLInputElement>>('cell');

  /**
   * Découpe la valeur courante en `length()` cases (chaîne vide pour les cases manquantes).
   */
  protected readonly slots = computed<string[]>(() => {
    const len = this.length();
    const v = this.value();
    const arr: string[] = [];
    for (let i = 0; i < len; i++) {
      arr.push(v[i] ?? '');
    }
    return arr;
  });

  constructor() {
    // Émet `complete` lorsque la valeur atteint la longueur cible.
    effect(() => {
      const v = this.value();
      if (v.length === this.length() && /^[0-9]+$/.test(v)) {
        this.complete.emit(v);
      }
    });
  }

  ngAfterViewInit(): void {
    // No-op : les références sont prêtes via `viewChildren`.
  }

  protected onInput(event: Event, index: number): void {
    const input = event.target as HTMLInputElement;
    const raw = (input.value ?? '').replace(/[^0-9]/g, '');
    const digit = raw.slice(-1); // ne garde que le dernier chiffre saisi

    const current = this.value();
    const arr = current.split('');
    while (arr.length < this.length()) arr.push('');
    arr[index] = digit;
    const next = arr.join('').slice(0, this.length());

    // Resync le DOM avec la valeur nettoyée (sinon "12" reste affiché).
    input.value = digit;

    this.value.set(next);

    if (digit && index < this.length() - 1) {
      this.focusCell(index + 1);
    }
  }

  protected onKeydown(event: KeyboardEvent, index: number): void {
    const input = event.target as HTMLInputElement;
    const arr = this.value().split('');
    while (arr.length < this.length()) arr.push('');

    if (event.key === 'Backspace') {
      if (arr[index]) {
        arr[index] = '';
        this.value.set(arr.join(''));
        return;
      }
      if (index > 0) {
        arr[index - 1] = '';
        this.value.set(arr.join(''));
        this.focusCell(index - 1);
        event.preventDefault();
      }
      return;
    }

    if (event.key === 'ArrowLeft' && index > 0) {
      this.focusCell(index - 1);
      event.preventDefault();
      return;
    }
    if (event.key === 'ArrowRight' && index < this.length() - 1) {
      this.focusCell(index + 1);
      event.preventDefault();
      return;
    }
  }

  protected onPaste(event: ClipboardEvent, index: number): void {
    event.preventDefault();
    const text = (event.clipboardData?.getData('text') ?? '')
      .replace(/[^0-9]/g, '')
      .slice(0, this.length() - index);
    if (!text) return;

    const arr = this.value().split('');
    while (arr.length < this.length()) arr.push('');
    for (let i = 0; i < text.length; i++) {
      arr[index + i] = text[i];
    }
    const next = arr.join('').slice(0, this.length());
    this.value.set(next);

    const nextFocus = Math.min(index + text.length, this.length() - 1);
    this.focusCell(nextFocus);
  }

  protected onFocus(event: FocusEvent): void {
    const input = event.target as HTMLInputElement;
    queueMicrotask(() => input.select?.());
  }

  private focusCell(index: number): void {
    const refs = this.cells();
    const el = refs[index]?.nativeElement;
    if (el) {
      el.focus();
      el.select?.();
    }
  }
}
