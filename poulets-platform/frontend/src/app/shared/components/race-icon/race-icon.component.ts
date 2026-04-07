import { Component, Input, ChangeDetectionStrategy } from '@angular/core';
import { CommonModule } from '@angular/common';
import { MatIconModule } from '@angular/material/icon';
import { MatTooltipModule } from '@angular/material/tooltip';
import { RaceLabelPipe } from '../../pipes/race-label.pipe';
import { Race } from '../../models/poulet.model';

const RACE_ICONS: Record<string, string> = {
  [Race.LOCAL]: 'egg_alt',
  [Race.BICYCLETTE]: 'directions_bike',
  [Race.BRAHMA]: 'pets',
  [Race.SUSSEX]: 'egg',
  [Race.RHODE_ISLAND]: 'egg_alt',
  [Race.LEGHORN]: 'egg',
  [Race.COUCOU]: 'nest_cam_wired_stand',
  [Race.PINTADE]: 'flutter_dash',
  [Race.DINDE]: 'set_meal',
  [Race.MIXED]: 'join_inner',
};

const RACE_COLORS: Record<string, string> = {
  [Race.LOCAL]: '#795548',
  [Race.BICYCLETTE]: '#ff5722',
  [Race.BRAHMA]: '#9c27b0',
  [Race.SUSSEX]: '#4caf50',
  [Race.RHODE_ISLAND]: '#f44336',
  [Race.LEGHORN]: '#ffc107',
  [Race.COUCOU]: '#607d8b',
  [Race.PINTADE]: '#3f51b5',
  [Race.DINDE]: '#ff9800',
  [Race.MIXED]: '#009688',
};

@Component({
  selector: 'app-race-icon',
  standalone: true,
  imports: [CommonModule, MatIconModule, MatTooltipModule, RaceLabelPipe],
  changeDetection: ChangeDetectionStrategy.OnPush,
  template: `
    <span
      class="race-icon"
      [style.background-color]="getColor()"
      [matTooltip]="race | raceLabel"
    >
      <mat-icon>{{ getIcon() }}</mat-icon>
    </span>
  `,
  styles: [`
    .race-icon {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 36px;
      height: 36px;
      border-radius: 50%;
      color: white;
    }

    .race-icon mat-icon {
      font-size: 20px;
      width: 20px;
      height: 20px;
    }
  `],
})
export class RaceIconComponent {
  @Input() race: string | Race = '';

  getIcon(): string {
    return RACE_ICONS[this.race] || 'egg_alt';
  }

  getColor(): string {
    return RACE_COLORS[this.race] || '#9e9e9e';
  }
}
