import {
  Component,
  OnInit,
  OnDestroy,
  inject,
  signal,
  computed,
  ChangeDetectionStrategy,
  PLATFORM_ID,
  NgZone,
  ElementRef,
  AfterViewInit,
} from '@angular/core';
import { isPlatformBrowser, CommonModule } from '@angular/common';
import { HttpClient } from '@angular/common/http';
import { Router } from '@angular/router';
import { MatIconModule } from '@angular/material/icon';
import { MatButtonModule } from '@angular/material/button';
import { MatSlideToggleModule } from '@angular/material/slide-toggle';
import { MatTooltipModule } from '@angular/material/tooltip';
import { TranslateModule } from '@ngx-translate/core';
import { AuthService } from '@app/core/services/auth.service';
import {
  Subject,
  interval,
  switchMap,
  catchError,
  of,
  takeUntil,
  startWith,
  tap,
} from 'rxjs';

/* ================================================================
   INTERFACES
   ================================================================ */
interface Annonce {
  id: string;
  race: string;
  raceColor: string;
  quantity: number;
  weightKg: number;
  pricePerKg: number;
  location: string;
  availability: string;
  status: 'available' | 'partial' | 'sold';
  eleveurId: string;
  eleveurName: string;
  createdAt: string;
  description?: string;
  certifications?: string[];
  rating?: number;
  phone?: string;
}

interface Besoin {
  id: string;
  type: 'restaurant' | 'menage' | 'evenement' | 'revendeur';
  race: string;
  quantity: number;
  minWeightKg: number;
  budgetPerKg: number;
  location: string;
  date: string;
  frequency: 'hebdo' | 'mensuel' | 'ponctuel';
  clientId: string;
  clientName: string;
  createdAt: string;
  description?: string;
  rating?: number;
  phone?: string;
}

interface Aliment {
  id: string;
  product: string;
  pricePerSac: number;
  stock: number;
  zone: string;
  targetRace: string;
  producteurId: string;
  producteurName: string;
  createdAt: string;
  description?: string;
  rating?: number;
  phone?: string;
}

interface Poussin {
  id: string;
  producteur: string;
  producteurId: string;
  race: string;
  ageJours: number;
  quantity: number;
  priceUnit: number;
  vaccinated: boolean;
  vaccinationDetails?: string;
  location: string;
  region: string;
  availableFrom: string;
  status: 'active' | 'reserve' | 'epuise';
  createdAt: string;
}

interface DashboardStats {
  activeMatchings: number;
  liveUsers: number;
  contractsToday: number;
  newBesoins: number;
}

interface ValidatedContract {
  id: string;
  eleveurName: string;
  clientName: string;
  quantity: number;
  race: string;
  frequency: string;
  validatedAt: string;
}

interface UpcomingDelivery {
  id: string;
  eleveurName: string;
  clientName: string;
  quantity: number;
  race: string;
  deliveryDate: string;
  daysLeft: number;
}

/* ================================================================
   MOCK DATA
   ================================================================ */
const MOCK_ANNONCES: Annonce[] = [
  { id: 'a1', race: 'Bicyclette', raceColor: '#E53935', quantity: 50, weightKg: 2.3, pricePerKg: 2500, location: 'Ouagadougou', availability: '2026-04-10', status: 'available', eleveurId: 'e1', eleveurName: 'Ibrahim Ouedraogo', createdAt: new Date(Date.now() - 20 * 60000).toISOString(), description: 'Poulets bicyclette bien nourris, elevage traditionnel.', rating: 4.5, phone: '+226 70 ** ** 33' },
  { id: 'a2', race: 'Brahma', raceColor: '#8E24AA', quantity: 30, weightKg: 3.5, pricePerKg: 3200, location: 'Bobo-Dioulasso', availability: '2026-04-12', status: 'available', eleveurId: 'e2', eleveurName: 'Amadou Sanou', createdAt: new Date(Date.now() - 5 * 60000).toISOString(), description: 'Brahma de qualite superieure, vaccination complete.', rating: 4.8, phone: '+226 76 ** ** 12' },
  { id: 'a3', race: 'Sussex', raceColor: '#43A047', quantity: 100, weightKg: 2.8, pricePerKg: 2800, location: 'Koudougou', availability: '2026-04-08', status: 'partial', eleveurId: 'e3', eleveurName: 'Fatimata Compaore', createdAt: new Date(Date.now() - 90 * 60000).toISOString(), description: 'Sussex robustes, elevage semi-intensif.', rating: 4.2, phone: '+226 71 ** ** 45' },
  { id: 'a4', race: 'Race locale', raceColor: '#FF8F00', quantity: 200, weightKg: 1.8, pricePerKg: 2000, location: 'Ouahigouya', availability: '2026-04-09', status: 'available', eleveurId: 'e4', eleveurName: 'Moussa Sawadogo', createdAt: new Date(Date.now() - 2 * 60000).toISOString(), description: 'Poulets locaux traditionnels du Nord.', rating: 4.0, phone: '+226 70 ** ** 78' },
  { id: 'a5', race: 'Rhode Island', raceColor: '#D84315', quantity: 75, weightKg: 3.0, pricePerKg: 3000, location: 'Banfora', availability: '2026-04-15', status: 'available', eleveurId: 'e5', eleveurName: 'Adama Traore', createdAt: new Date(Date.now() - 45 * 60000).toISOString(), description: 'Rhode Island premium, alimentation bio.', rating: 4.6, phone: '+226 66 ** ** 90' },
  { id: 'a6', race: 'Leghorn', raceColor: '#1E88E5', quantity: 60, weightKg: 2.1, pricePerKg: 2200, location: 'Tenkodogo', availability: '2026-04-11', status: 'sold', eleveurId: 'e6', eleveurName: 'Hamidou Zoungrana', createdAt: new Date(Date.now() - 120 * 60000).toISOString(), description: 'Leghorn pondeuses reformees.', rating: 3.8, phone: '+226 72 ** ** 55' },
  { id: 'a7', race: 'Coucou', raceColor: '#6D4C41', quantity: 40, weightKg: 2.6, pricePerKg: 2700, location: 'Kaya', availability: '2026-04-13', status: 'available', eleveurId: 'e7', eleveurName: 'Salif Kabore', createdAt: new Date(Date.now() - 10 * 60000).toISOString(), description: 'Coucou de Malines, rustiques.', rating: 4.3, phone: '+226 75 ** ** 21' },
  { id: 'a8', race: 'Pintade', raceColor: '#546E7A', quantity: 120, weightKg: 1.5, pricePerKg: 3500, location: 'Ziniar\u00e9', availability: '2026-04-14', status: 'partial', eleveurId: 'e8', eleveurName: 'Rasmane Ilboudo', createdAt: new Date(Date.now() - 30 * 60000).toISOString(), description: 'Pintades de brousse, gout authentique.', rating: 4.7, phone: '+226 70 ** ** 67' },
  { id: 'a9', race: 'Dinde', raceColor: '#7B1FA2', quantity: 25, weightKg: 5.5, pricePerKg: 4000, location: 'Dedougou', availability: '2026-04-16', status: 'available', eleveurId: 'e9', eleveurName: 'Boureima Belem', createdAt: new Date(Date.now() - 55 * 60000).toISOString(), description: 'Dindes de grande taille pour evenements.', rating: 4.1, phone: '+226 68 ** ** 44' },
  { id: 'a10', race: 'Bicyclette', raceColor: '#E53935', quantity: 80, weightKg: 2.0, pricePerKg: 2300, location: 'Fada', availability: '2026-04-10', status: 'available', eleveurId: 'e10', eleveurName: 'Issa Tamboura', createdAt: new Date(Date.now() - 8 * 60000).toISOString(), description: 'Bicyclette du terroir de lEst.', rating: 4.4, phone: '+226 77 ** ** 88' },
  { id: 'a11', race: 'Sussex', raceColor: '#43A047', quantity: 45, weightKg: 2.9, pricePerKg: 2900, location: 'Leo', availability: '2026-04-17', status: 'available', eleveurId: 'e11', eleveurName: 'Pascal Somda', createdAt: new Date(Date.now() - 75 * 60000).toISOString(), description: 'Sussex du Sud-Ouest.', rating: 4.0, phone: '+226 73 ** ** 99' },
  { id: 'a12', race: 'Race locale', raceColor: '#FF8F00', quantity: 150, weightKg: 1.9, pricePerKg: 1900, location: 'Manga', availability: '2026-04-09', status: 'partial', eleveurId: 'e12', eleveurName: 'Tasser\u00e9 Nikiema', createdAt: new Date(Date.now() - 15 * 60000).toISOString(), description: 'Poulets locaux du Centre-Sud.', rating: 3.9, phone: '+226 74 ** ** 11' },
  { id: 'a13', race: 'Poulet de chair', raceColor: '#FF5722', quantity: 300, weightKg: 2.5, pricePerKg: 2100, location: 'Ouagadougou', availability: '2026-04-11', status: 'available', eleveurId: 'e13', eleveurName: 'Abdoulaye Diallo', createdAt: new Date(Date.now() - 35 * 60000).toISOString(), description: 'Chair industrielle, lot homogene.', rating: 4.3, phone: '+226 70 ** ** 44' },
  { id: 'a14', race: 'Poule pondeuse', raceColor: '#AB47BC', quantity: 90, weightKg: 1.8, pricePerKg: 1800, location: 'Bobo-Dioulasso', availability: '2026-04-13', status: 'available', eleveurId: 'e14', eleveurName: 'Mariam Sankara', createdAt: new Date(Date.now() - 42 * 60000).toISOString(), description: 'Pondeuses reformees, bonne chair.', rating: 4.1, phone: '+226 76 ** ** 33' },
  { id: 'a15', race: 'Brahma', raceColor: '#8E24AA', quantity: 20, weightKg: 4.0, pricePerKg: 3400, location: 'Koudougou', availability: '2026-04-14', status: 'available', eleveurId: 'e15', eleveurName: 'Seydou Kone', createdAt: new Date(Date.now() - 12 * 60000).toISOString(), description: 'Brahma geants, excellente genetique.', rating: 4.9, phone: '+226 71 ** ** 88' },
  { id: 'a16', race: 'Race locale', raceColor: '#FF8F00', quantity: 65, weightKg: 1.6, pricePerKg: 1950, location: 'Ouahigouya', availability: '2026-04-10', status: 'available', eleveurId: 'e16', eleveurName: 'Harouna Yameogo', createdAt: new Date(Date.now() - 50 * 60000).toISOString(), description: 'Poulets bio du Nord.', rating: 4.0, phone: '+226 70 ** ** 21' },
  { id: 'a17', race: 'Pintade', raceColor: '#546E7A', quantity: 85, weightKg: 1.4, pricePerKg: 3600, location: 'Banfora', availability: '2026-04-15', status: 'partial', eleveurId: 'e17', eleveurName: 'Lassina Coulibaly', createdAt: new Date(Date.now() - 18 * 60000).toISOString(), description: 'Pintades Cascades, saveur unique.', rating: 4.5, phone: '+226 66 ** ** 57' },
  { id: 'a18', race: 'Bicyclette', raceColor: '#E53935', quantity: 110, weightKg: 2.2, pricePerKg: 2400, location: 'Tenkodogo', availability: '2026-04-12', status: 'available', eleveurId: 'e18', eleveurName: 'Ousmane Tiendrebeogo', createdAt: new Date(Date.now() - 65 * 60000).toISOString(), description: 'Bicyclette robuste, grand format.', rating: 4.2, phone: '+226 72 ** ** 66' },
  { id: 'a19', race: 'Sussex', raceColor: '#43A047', quantity: 55, weightKg: 2.7, pricePerKg: 2850, location: 'Kaya', availability: '2026-04-16', status: 'available', eleveurId: 'e19', eleveurName: 'Amidou Ouattara', createdAt: new Date(Date.now() - 28 * 60000).toISOString(), description: 'Sussex elevage familial.', rating: 4.4, phone: '+226 75 ** ** 09' },
  { id: 'a20', race: 'Dinde', raceColor: '#7B1FA2', quantity: 15, weightKg: 6.0, pricePerKg: 4200, location: 'Ziniar\u00e9', availability: '2026-04-18', status: 'available', eleveurId: 'e20', eleveurName: 'Karim Zongo', createdAt: new Date(Date.now() - 72 * 60000).toISOString(), description: 'Dindes blanches, evenements speciaux.', rating: 4.6, phone: '+226 70 ** ** 95' },
  { id: 'a21', race: 'Coucou', raceColor: '#6D4C41', quantity: 35, weightKg: 2.4, pricePerKg: 2650, location: 'Dedougou', availability: '2026-04-11', status: 'available', eleveurId: 'e21', eleveurName: 'Boukary Sore', createdAt: new Date(Date.now() - 38 * 60000).toISOString(), description: 'Coucou du Mouhoun.', rating: 4.1, phone: '+226 68 ** ** 22' },
  { id: 'a22', race: 'Poulet de chair', raceColor: '#FF5722', quantity: 250, weightKg: 2.3, pricePerKg: 2050, location: 'Fada N\'Gourma', availability: '2026-04-14', status: 'available', eleveurId: 'e22', eleveurName: 'Dramane Bationo', createdAt: new Date(Date.now() - 82 * 60000).toISOString(), description: 'Chair standard Est.', rating: 3.9, phone: '+226 77 ** ** 41' },
  { id: 'a23', race: 'Race locale', raceColor: '#FF8F00', quantity: 70, weightKg: 1.7, pricePerKg: 2100, location: 'Leo', availability: '2026-04-09', status: 'available', eleveurId: 'e23', eleveurName: 'Idrissa Pare', createdAt: new Date(Date.now() - 95 * 60000).toISOString(), description: 'Poulets du Sud-Ouest.', rating: 4.0, phone: '+226 73 ** ** 18' },
  { id: 'a24', race: 'Leghorn', raceColor: '#1E88E5', quantity: 40, weightKg: 2.0, pricePerKg: 2150, location: 'Manga', availability: '2026-04-12', status: 'available', eleveurId: 'e24', eleveurName: 'Ali Diande', createdAt: new Date(Date.now() - 105 * 60000).toISOString(), description: 'Leghorn Centre-Sud.', rating: 3.7, phone: '+226 74 ** ** 55' },
];

const MOCK_BESOINS: Besoin[] = [
  { id: 'b1', type: 'restaurant', race: 'Bicyclette', quantity: 30, minWeightKg: 2.0, budgetPerKg: 2800, location: 'Ouagadougou', date: '2026-04-12', frequency: 'hebdo', clientId: 'c1', clientName: 'Restaurant Le Baobab', createdAt: new Date(Date.now() - 15 * 60000).toISOString(), description: 'Besoin regulier pour restaurant 50 couverts.', rating: 4.3, phone: '+226 70 ** ** 56' },
  { id: 'b2', type: 'evenement', race: 'Brahma', quantity: 100, minWeightKg: 3.0, budgetPerKg: 3500, location: 'Bobo-Dioulasso', date: '2026-04-20', frequency: 'ponctuel', clientId: 'c2', clientName: 'Mariage Ouattara', createdAt: new Date(Date.now() - 3 * 60000).toISOString(), description: 'Grande ceremonie, 500 invites.', rating: 0, phone: '+226 76 ** ** 89' },
  { id: 'b3', type: 'menage', race: 'Race locale', quantity: 5, minWeightKg: 1.5, budgetPerKg: 2200, location: 'Ouagadougou', date: '2026-04-08', frequency: 'mensuel', clientId: 'c3', clientName: 'Famille Zongo', createdAt: new Date(Date.now() - 40 * 60000).toISOString(), description: 'Achat mensuel pour famille de 8 personnes.', rating: 4.1, phone: '+226 71 ** ** 34' },
  { id: 'b4', type: 'restaurant', race: 'Pintade', quantity: 20, minWeightKg: 1.3, budgetPerKg: 3800, location: 'Koudougou', date: '2026-04-10', frequency: 'hebdo', clientId: 'c4', clientName: 'Maquis Chez Tanti', createdAt: new Date(Date.now() - 25 * 60000).toISOString(), description: 'Pintades pour grillade chaque week-end.', rating: 4.6, phone: '+226 70 ** ** 72' },
  { id: 'b5', type: 'revendeur', race: 'Sussex', quantity: 200, minWeightKg: 2.5, budgetPerKg: 2600, location: 'Ouagadougou', date: '2026-04-15', frequency: 'hebdo', clientId: 'c5', clientName: 'Marche Rood Woko', createdAt: new Date(Date.now() - 60 * 60000).toISOString(), description: 'Revendeur marche central, gros volume.', rating: 4.0, phone: '+226 66 ** ** 23' },
  { id: 'b6', type: 'evenement', race: 'Dinde', quantity: 15, minWeightKg: 5.0, budgetPerKg: 4200, location: 'Banfora', date: '2026-04-25', frequency: 'ponctuel', clientId: 'c6', clientName: 'Bapteme Hema', createdAt: new Date(Date.now() - 10 * 60000).toISOString(), description: 'Bapteme traditionnel avec dindes.', rating: 0, phone: '+226 68 ** ** 41' },
  { id: 'b7', type: 'restaurant', race: 'Bicyclette', quantity: 50, minWeightKg: 2.2, budgetPerKg: 2700, location: 'Bobo-Dioulasso', date: '2026-04-11', frequency: 'hebdo', clientId: 'c7', clientName: 'Hotel Relais', createdAt: new Date(Date.now() - 50 * 60000).toISOString(), description: 'Hotel 3 etoiles, qualite exigee.', rating: 4.5, phone: '+226 76 ** ** 55' },
  { id: 'b8', type: 'menage', race: 'Race locale', quantity: 3, minWeightKg: 1.8, budgetPerKg: 2100, location: 'Kaya', date: '2026-04-09', frequency: 'ponctuel', clientId: 'c8', clientName: 'Famille Sawadogo', createdAt: new Date(Date.now() - 35 * 60000).toISOString(), description: 'Achat pour fete familiale.', rating: 3.8, phone: '+226 75 ** ** 63' },
  { id: 'b9', type: 'restaurant', race: 'Coucou', quantity: 15, minWeightKg: 2.4, budgetPerKg: 3000, location: 'Ouahigouya', date: '2026-04-14', frequency: 'mensuel', clientId: 'c9', clientName: 'Brasserie du Nord', createdAt: new Date(Date.now() - 5 * 60000).toISOString(), description: 'Restaurant specialise poulet braise.', rating: 4.2, phone: '+226 70 ** ** 17' },
  { id: 'b10', type: 'evenement', race: 'Brahma', quantity: 80, minWeightKg: 3.5, budgetPerKg: 3300, location: 'Ouagadougou', date: '2026-04-30', frequency: 'ponctuel', clientId: 'c10', clientName: 'Conf. UEMOA', createdAt: new Date(Date.now() - 70 * 60000).toISOString(), description: 'Conference internationale, traiteur.', rating: 4.7, phone: '+226 70 ** ** 01' },
  { id: 'b11', type: 'menage', race: 'Bicyclette', quantity: 10, minWeightKg: 2.0, budgetPerKg: 2500, location: 'Tenkodogo', date: '2026-04-12', frequency: 'mensuel', clientId: 'c11', clientName: 'Famille Ouedraogo', createdAt: new Date(Date.now() - 22 * 60000).toISOString(), description: 'Commande mensuelle reguliere.', rating: 4.0, phone: '+226 72 ** ** 38' },
  { id: 'b12', type: 'restaurant', race: 'Leghorn', quantity: 25, minWeightKg: 1.8, budgetPerKg: 2400, location: 'Dedougou', date: '2026-04-13', frequency: 'hebdo', clientId: 'c12', clientName: 'Resto La Terrasse', createdAt: new Date(Date.now() - 18 * 60000).toISOString(), description: 'Petit restaurant, budget serre.', rating: 3.5, phone: '+226 68 ** ** 77' },
  { id: 'b13', type: 'revendeur', race: 'Poulet de chair', quantity: 500, minWeightKg: 2.0, budgetPerKg: 2200, location: 'Ouagadougou', date: '2026-04-14', frequency: 'hebdo', clientId: 'c13', clientName: 'Supermarche Marina', createdAt: new Date(Date.now() - 48 * 60000).toISOString(), description: 'Grande surface, volume important.', rating: 4.4, phone: '+226 70 ** ** 88' },
  { id: 'b14', type: 'restaurant', race: 'Race locale', quantity: 40, minWeightKg: 1.5, budgetPerKg: 2300, location: 'Bobo-Dioulasso', date: '2026-04-11', frequency: 'hebdo', clientId: 'c14', clientName: 'Grill Express', createdAt: new Date(Date.now() - 32 * 60000).toISOString(), description: 'Fast food poulet grille.', rating: 4.1, phone: '+226 76 ** ** 44' },
  { id: 'b15', type: 'evenement', race: 'Pintade', quantity: 60, minWeightKg: 1.3, budgetPerKg: 3700, location: 'Koudougou', date: '2026-04-22', frequency: 'ponctuel', clientId: 'c15', clientName: 'Fete du Mogho', createdAt: new Date(Date.now() - 55 * 60000).toISOString(), description: 'Festival culturel annuel.', rating: 0, phone: '+226 71 ** ** 29' },
  { id: 'b16', type: 'menage', race: 'Poule pondeuse', quantity: 8, minWeightKg: 1.6, budgetPerKg: 2000, location: 'Ouahigouya', date: '2026-04-10', frequency: 'mensuel', clientId: 'c16', clientName: 'Famille Diallo', createdAt: new Date(Date.now() - 14 * 60000).toISOString(), description: 'Pondeuses pour oeufs familiaux.', rating: 3.9, phone: '+226 70 ** ** 62' },
  { id: 'b17', type: 'restaurant', race: 'Bicyclette', quantity: 35, minWeightKg: 2.0, budgetPerKg: 2600, location: 'Banfora', date: '2026-04-12', frequency: 'hebdo', clientId: 'c17', clientName: 'Maquis Le Fromager', createdAt: new Date(Date.now() - 85 * 60000).toISOString(), description: 'Restaurant du sud.', rating: 4.3, phone: '+226 66 ** ** 71' },
  { id: 'b18', type: 'revendeur', race: 'Race locale', quantity: 150, minWeightKg: 1.5, budgetPerKg: 2100, location: 'Tenkodogo', date: '2026-04-15', frequency: 'mensuel', clientId: 'c18', clientName: 'Marche Central Tenkodogo', createdAt: new Date(Date.now() - 92 * 60000).toISOString(), description: 'Revendeur regional.', rating: 3.8, phone: '+226 72 ** ** 15' },
  { id: 'b19', type: 'menage', race: 'Brahma', quantity: 4, minWeightKg: 3.0, budgetPerKg: 3200, location: 'Kaya', date: '2026-04-09', frequency: 'ponctuel', clientId: 'c19', clientName: 'Famille Kabore', createdAt: new Date(Date.now() - 7 * 60000).toISOString(), description: 'Achat exceptionnel ceremonie.', rating: 4.0, phone: '+226 75 ** ** 47' },
  { id: 'b20', type: 'restaurant', race: 'Sussex', quantity: 45, minWeightKg: 2.5, budgetPerKg: 2800, location: 'Ziniar\u00e9', date: '2026-04-13', frequency: 'hebdo', clientId: 'c20', clientName: 'Auberge Naaba', createdAt: new Date(Date.now() - 62 * 60000).toISOString(), description: 'Auberge touristique.', rating: 4.5, phone: '+226 70 ** ** 83' },
  { id: 'b21', type: 'evenement', race: 'Dinde', quantity: 25, minWeightKg: 5.0, budgetPerKg: 4500, location: 'Ouagadougou', date: '2026-04-28', frequency: 'ponctuel', clientId: 'c21', clientName: 'Gala Presidentiel', createdAt: new Date(Date.now() - 110 * 60000).toISOString(), description: 'Gala officiel.', rating: 4.8, phone: '+226 70 ** ** 02' },
  { id: 'b22', type: 'restaurant', race: 'Pintade', quantity: 30, minWeightKg: 1.3, budgetPerKg: 3900, location: 'Dedougou', date: '2026-04-14', frequency: 'hebdo', clientId: 'c22', clientName: 'Chez Moumouni', createdAt: new Date(Date.now() - 27 * 60000).toISOString(), description: 'Specialite pintade.', rating: 4.2, phone: '+226 68 ** ** 93' },
];

const MOCK_ALIMENTS: Aliment[] = [
  { id: 'al1', product: 'Aliment demarrage', pricePerSac: 12500, stock: 150, zone: 'Centre', targetRace: 'Toutes races', producteurId: 'p1', producteurName: 'Faso Nutrition', createdAt: new Date(Date.now() - 30 * 60000).toISOString(), description: 'Aliment demarrage 0-4 semaines, sac de 50kg.', rating: 4.5, phone: '+226 70 ** ** 90' },
  { id: 'al2', product: 'Aliment croissance', pricePerSac: 11000, stock: 200, zone: 'Hauts-Bassins', targetRace: 'Brahma/Sussex', producteurId: 'p2', producteurName: 'SOFAB', createdAt: new Date(Date.now() - 10 * 60000).toISOString(), description: 'Aliment croissance 4-8 semaines, haute proteine.', rating: 4.3, phone: '+226 76 ** ** 45' },
  { id: 'al3', product: 'Aliment finition', pricePerSac: 10500, stock: 80, zone: 'Centre', targetRace: 'Toutes races', producteurId: 'p3', producteurName: 'ProFeed BF', createdAt: new Date(Date.now() - 55 * 60000).toISOString(), description: 'Aliment finition 8+ semaines, engraissement.', rating: 4.1, phone: '+226 71 ** ** 22' },
  { id: 'al4', product: 'Concentre proteique', pricePerSac: 18000, stock: 45, zone: 'Centre-Ouest', targetRace: 'Race locale', producteurId: 'p4', producteurName: 'Agri Plus', createdAt: new Date(Date.now() - 5 * 60000).toISOString(), description: 'Complement proteique pour poulets locaux.', rating: 4.6, phone: '+226 70 ** ** 66' },
  { id: 'al5', product: 'Premix vitamine', pricePerSac: 8500, stock: 300, zone: 'Nord', targetRace: 'Toutes races', producteurId: 'p5', producteurName: 'VitaPoule', createdAt: new Date(Date.now() - 20 * 60000).toISOString(), description: 'Premix vitamines et mineraux, sac de 25kg.', rating: 4.4, phone: '+226 70 ** ** 14' },
  { id: 'al6', product: 'Aliment pondeuse', pricePerSac: 13000, stock: 90, zone: 'Cascades', targetRace: 'Leghorn', producteurId: 'p6', producteurName: 'Poulet Sante', createdAt: new Date(Date.now() - 40 * 60000).toISOString(), description: 'Aliment special pondeuses, calcium renforce.', rating: 4.2, phone: '+226 66 ** ** 08' },
  { id: 'al7', product: 'Son de ble', pricePerSac: 5000, stock: 500, zone: 'Boucle du Mouhoun', targetRace: 'Toutes races', producteurId: 'p7', producteurName: 'Minoterie Faso', createdAt: new Date(Date.now() - 15 * 60000).toISOString(), description: 'Son de ble, complement alimentaire economique.', rating: 3.9, phone: '+226 68 ** ** 33' },
  { id: 'al8', product: 'Tourteau soja', pricePerSac: 15000, stock: 60, zone: 'Hauts-Bassins', targetRace: 'Brahma', producteurId: 'p8', producteurName: 'Soja Ouest', createdAt: new Date(Date.now() - 48 * 60000).toISOString(), description: 'Tourteau de soja local, riche en proteines.', rating: 4.0, phone: '+226 76 ** ** 71' },
  { id: 'al9', product: 'Aliment poussin', pricePerSac: 14000, stock: 120, zone: 'Centre', targetRace: 'Toutes races', producteurId: 'p9', producteurName: 'Faso Nutrition', createdAt: new Date(Date.now() - 25 * 60000).toISOString(), description: 'Aliment miette pour poussins.', rating: 4.5, phone: '+226 70 ** ** 90' },
  { id: 'al10', product: 'Mais concasse', pricePerSac: 7500, stock: 400, zone: 'Centre-Ouest', targetRace: 'Race locale', producteurId: 'p10', producteurName: 'Cereales BF', createdAt: new Date(Date.now() - 33 * 60000).toISOString(), description: 'Mais concasse, complement energetique.', rating: 4.0, phone: '+226 71 ** ** 55' },
  { id: 'al11', product: 'Anti-stress volaille', pricePerSac: 9500, stock: 75, zone: 'Nord', targetRace: 'Toutes races', producteurId: 'p11', producteurName: 'VitaPoule', createdAt: new Date(Date.now() - 58 * 60000).toISOString(), description: 'Additif anti-stress et vitamines.', rating: 4.3, phone: '+226 70 ** ** 14' },
  { id: 'al12', product: 'Aliment reproducteur', pricePerSac: 16000, stock: 35, zone: 'Hauts-Bassins', targetRace: 'Brahma/Sussex', producteurId: 'p12', producteurName: 'SOFAB', createdAt: new Date(Date.now() - 68 * 60000).toISOString(), description: 'Aliment pour reproducteurs.', rating: 4.4, phone: '+226 76 ** ** 45' },
];

const MOCK_POUSSINS: Poussin[] = [
  { id: 'pou1', producteur: 'Couvoir National de Ouaga', producteurId: 'couv-001', race: 'Race locale', ageJours: 1, quantity: 500, priceUnit: 750, vaccinated: true, vaccinationDetails: 'Marek + Newcastle HB1', location: 'Ouagadougou', region: 'Centre', availableFrom: '2026-04-15', status: 'active', createdAt: new Date(Date.now() - 10 * 60000).toISOString() },
  { id: 'pou2', producteur: 'Couvoir Moderne de Bobo', producteurId: 'couv-002', race: 'Brahma', ageJours: 7, quantity: 300, priceUnit: 1450, vaccinated: true, vaccinationDetails: 'Marek + Newcastle HB1', location: 'Bobo-Dioulasso', region: 'Hauts-Bassins', availableFrom: '2026-04-20', status: 'active', createdAt: new Date(Date.now() - 25 * 60000).toISOString() },
  { id: 'pou3', producteur: 'Aviculture du Sahel', producteurId: 'couv-003', race: 'Pintade', ageJours: 14, quantity: 200, priceUnit: 1250, vaccinated: true, vaccinationDetails: 'Newcastle HB1', location: 'Dori', region: 'Sahel', availableFrom: '2026-04-25', status: 'active', createdAt: new Date(Date.now() - 5 * 60000).toISOString() },
  { id: 'pou4', producteur: 'SOFAB Poussins', producteurId: 'couv-004', race: 'Poulet de chair', ageJours: 21, quantity: 1000, priceUnit: 1300, vaccinated: true, vaccinationDetails: 'Marek + Newcastle HB1 + Gumboro', location: 'Ouagadougou', region: 'Centre', availableFrom: '2026-05-01', status: 'active', createdAt: new Date(Date.now() - 40 * 60000).toISOString() },
  { id: 'pou5', producteur: 'Couvoir Faso Koko', producteurId: 'couv-005', race: 'Poule pondeuse', ageJours: 1, quantity: 2000, priceUnit: 950, vaccinated: true, vaccinationDetails: 'Marek', location: 'Ouagadougou', region: 'Centre', availableFrom: '2026-05-05', status: 'active', createdAt: new Date(Date.now() - 15 * 60000).toISOString() },
  { id: 'pou6', producteur: 'Poussins du Houet', producteurId: 'couv-006', race: 'Brahma', ageJours: 14, quantity: 150, priceUnit: 1650, vaccinated: true, vaccinationDetails: 'Marek + Newcastle HB1', location: 'Bobo-Dioulasso', region: 'Hauts-Bassins', availableFrom: '2026-04-18', status: 'active', createdAt: new Date(Date.now() - 35 * 60000).toISOString() },
  { id: 'pou7', producteur: 'Couvoir de la Comoe', producteurId: 'couv-007', race: 'Race locale', ageJours: 7, quantity: 800, priceUnit: 680, vaccinated: true, vaccinationDetails: 'Marek + Newcastle HB1', location: 'Banfora', region: 'Cascades', availableFrom: '2026-04-22', status: 'active', createdAt: new Date(Date.now() - 50 * 60000).toISOString() },
  { id: 'pou8', producteur: 'Koudougou Aviculture', producteurId: 'couv-008', race: 'Poulet de chair', ageJours: 1, quantity: 3000, priceUnit: 700, vaccinated: false, location: 'Koudougou', region: 'Centre-Ouest', availableFrom: '2026-04-28', status: 'active', createdAt: new Date(Date.now() - 8 * 60000).toISOString() },
  { id: 'pou9', producteur: 'Couvoir Excellence BF', producteurId: 'couv-009', race: 'Poule pondeuse', ageJours: 21, quantity: 500, priceUnit: 1550, vaccinated: true, vaccinationDetails: 'Marek + Newcastle HB1 + Gumboro', location: 'Ouagadougou', region: 'Centre', availableFrom: '2026-05-03', status: 'active', createdAt: new Date(Date.now() - 18 * 60000).toISOString() },
  { id: 'pou10', producteur: 'Poussins du Centre', producteurId: 'couv-010', race: 'Pintade', ageJours: 7, quantity: 400, priceUnit: 1050, vaccinated: true, vaccinationDetails: 'Newcastle HB1', location: 'Ouagadougou', region: 'Centre', availableFrom: '2026-05-08', status: 'active', createdAt: new Date(Date.now() - 22 * 60000).toISOString() },
  { id: 'pou11', producteur: 'Sahel Couvoir', producteurId: 'couv-011', race: 'Race locale', ageJours: 21, quantity: 250, priceUnit: 1150, vaccinated: true, vaccinationDetails: 'Marek + Newcastle HB1', location: 'Dori', region: 'Sahel', availableFrom: '2026-04-16', status: 'active', createdAt: new Date(Date.now() - 60 * 60000).toISOString() },
  { id: 'pou12', producteur: 'Couvoir Wend-Panga', producteurId: 'couv-012', race: 'Brahma', ageJours: 1, quantity: 600, priceUnit: 1250, vaccinated: true, vaccinationDetails: 'Marek', location: 'Ouagadougou', region: 'Centre', availableFrom: '2026-04-21', status: 'active', createdAt: new Date(Date.now() - 3 * 60000).toISOString() },
  { id: 'pou13', producteur: 'Aviculture Moderne Ouaga', producteurId: 'couv-013', race: 'Poulet de chair', ageJours: 14, quantity: 1500, priceUnit: 1100, vaccinated: true, vaccinationDetails: 'Marek + Newcastle HB1 + Gumboro', location: 'Ouagadougou', region: 'Centre', availableFrom: '2026-04-26', status: 'active', createdAt: new Date(Date.now() - 45 * 60000).toISOString() },
  { id: 'pou14', producteur: 'Couvoir du Kadiogo', producteurId: 'couv-014', race: 'Poule pondeuse', ageJours: 7, quantity: 350, priceUnit: 1100, vaccinated: true, vaccinationDetails: 'Marek + Newcastle HB1', location: 'Ouagadougou', region: 'Centre', availableFrom: '2026-05-02', status: 'active', createdAt: new Date(Date.now() - 30 * 60000).toISOString() },
  { id: 'pou15', producteur: 'Poussins Premium Faso', producteurId: 'couv-015', race: 'Pintade', ageJours: 21, quantity: 180, priceUnit: 1400, vaccinated: true, vaccinationDetails: 'Newcastle HB1 + Gumboro', location: 'Ouagadougou', region: 'Centre', availableFrom: '2026-05-06', status: 'active', createdAt: new Date(Date.now() - 55 * 60000).toISOString() },
];

const MOCK_STATS: DashboardStats = {
  activeMatchings: 23,
  liveUsers: 847,
  contractsToday: 7,
  newBesoins: 14,
};

const MOCK_VALIDATED_CONTRACTS: ValidatedContract[] = [
  { id: 'vc1', eleveurName: 'Ibrahim Ouedraogo', clientName: 'Restaurant Le Baobab', quantity: 30, race: 'Bicyclette', frequency: 'hebdo', validatedAt: new Date(Date.now() - 2 * 60000).toISOString() },
  { id: 'vc2', eleveurName: 'Amadou Sanou', clientName: 'Hotel Relais', quantity: 50, race: 'Brahma', frequency: 'mensuel', validatedAt: new Date(Date.now() - 8 * 60000).toISOString() },
  { id: 'vc3', eleveurName: 'Fatimata Compaore', clientName: 'Marche Rood Woko', quantity: 100, race: 'Sussex', frequency: 'hebdo', validatedAt: new Date(Date.now() - 15 * 60000).toISOString() },
];

const MOCK_UPCOMING_DELIVERIES: UpcomingDelivery[] = [
  { id: 'ud1', eleveurName: 'Moussa Sawadogo', clientName: 'Famille Zongo', quantity: 30, race: 'Race locale', deliveryDate: '2026-04-10', daysLeft: 3 },
  { id: 'ud2', eleveurName: 'Salif Kabore', clientName: 'Brasserie du Nord', quantity: 15, race: 'Coucou', deliveryDate: '2026-04-09', daysLeft: 2 },
  { id: 'ud3', eleveurName: 'Adama Traore', clientName: 'Maquis Chez Tanti', quantity: 20, race: 'Pintade', deliveryDate: '2026-04-12', daysLeft: 5 },
  { id: 'ud4', eleveurName: 'Ibrahim Ouedraogo', clientName: 'Restaurant Le Baobab', quantity: 30, race: 'Bicyclette', deliveryDate: '2026-04-08', daysLeft: 1 },
  { id: 'ud5', eleveurName: 'Rasmane Ilboudo', clientName: 'Supermarche Marina', quantity: 50, race: 'Pintade', deliveryDate: '2026-04-13', daysLeft: 6 },
];

const REGIONS = [
  'Centre',
  'Hauts-Bassins',
  'Centre-Ouest',
  'Nord',
  'Cascades',
  'Centre-Est',
  'Centre-Nord',
  'Boucle du Mouhoun',
  'Est',
  'Plateau-Central',
  'Sud-Ouest',
  'Sahel',
  'Centre-Sud',
];

const VILLES = [
  'Ouagadougou',
  'Bobo-Dioulasso',
  'Koudougou',
  'Ouahigouya',
  'Banfora',
  'Tenkodogo',
  'Kaya',
  'D\u00e9dougou',
  'Fada N\'Gourma',
  'Ziniar\u00e9',
  'L\u00e9o',
  'Manga',
  'Dori',
];

const RACES_FILTER = [
  'Toutes',
  'Race locale',
  'Brahma',
  'Pintade',
  'Poulet de chair',
  'Poule pondeuse',
  'Bicyclette',
  'Sussex',
  'Coucou',
  'Leghorn',
  'Rhode Island',
  'Dinde',
];

// Base64 notification ding sound (very short sine wave beep)
const NOTIFICATION_SOUND_BASE64 = 'data:audio/wav;base64,UklGRl4EAABXQVZFZm10IBAAAAABAAEARKwAAIhYAQACABAAZGF0YToEAABiAKoA5QATADUASABRAF8AXABGAC8AFwD+/+f/0v++/67/ov+Z/5X/lP+Y/57/qv+4/8r/3f/y/wgAHgA0AEcAWABmAHEAeAB7AHoAdABqAFwATAA5ACQADgD3/+D/yf+z/57/jP9+/3T/bv9t/3H/ef+G/5f/rP/E/97/+v8XADQAUQBRAEQALQAWAP//6f/U/8L/sv+m/57/m/+c/6H/rP+7/87/5P/+/xkANgBTAEgANAAcAAUA7v/Y/8P/sP+h/5b/jv+L/4z/kv+d/6z/v//W//D/CwAnAEQAYQBWAEMAKgARAP3/5P/P/7v/rP+f/5f/k/+T/5j/of+v/8H/1v/u/wgAJABBAF4AVABDACoAEgD7/+T/z/+8/6z/oP+Y/5P/k/+Y/6H/r//B/9b/7v8JACUAQABWAE4APQAZAAAA5P/M/7r/qf+e/5P/jv+L/47/kf+c/6v/wP/Y//X/EwAxAE4AUQBCACoAEQD2/93/yP+z/6T/l/+R/4v/i/+P/5r/qf+9/9T/7/8MACoARABWAEoAOAAhAAcA8P/X/8H/r/+h/5b/j/+N/5D/l/+k/7T/yP/h//z/GQA3AFMAUwBEACwAFAD7/+P/zf+5/6n/nP+V/5H/kv+X/6D/rv/A/9X/7v8IACQAQABVAE4APAAlAA4A+P/g/8v/t/+o/5z/lP+Q/5D/lf+e/6v/vP/R/+r/BQAiAD8AWwBVAEQALQAUAP3/5f/P/7v/rP+f/5f/kv+S/5b/n/+s/7z/0P/o/wMAIAA9AFkAUgBBACoAEQD7/+P/zf+5/6r/nP+U/5D/kP+V/5z/qP+5/8z/5P/+/xsAOABTAFEAQgArABMA+//j/83/uf+q/5z/lf+R/5D/lP+c/6j/uf/M/+T//f8aADcAUgBQAEEAKgASAPv/4//N/7n/qv+c/5X/kP+Q/5T/m/+n/7n/zP/j//3/GgA3AFIAUABBACoAEgD7/+P/zf+6/6r/nP+V/5H/kP+U/5z/qP+5/83/5P/9/xoANwBSAFAAQQAqABIA+//j/83/uf+q/5z/lf+Q/5D/lf+c/6f/uP/M/+T//f8aADcAUgBQAEIAKwASAPz/5P/O/7r/q/+d/5X/kf+Q/5T/nP+n/7j/y//j//3/GgA3AFIAUABCAC0AFAD+/+b/0P+8/63/n/+X/5L/kv+V/53/qP+4/8v/4v/8/xgANQBQAE8AQQArABQA/v/m/9H/vP+u/6H/mf+U/5P/lv+e/6n/uf/L/+H/+v8WADMATgBOAD8AKgATAP3/5f/Q/7z/rf+g/5j/kv+R/5X/nP+n/7f/yf/g//n/FQASAA==';

/* ================================================================
   COMPONENT
   ================================================================ */
@Component({
  selector: 'app-public-dashboard',
  standalone: true,
  changeDetection: ChangeDetectionStrategy.OnPush,
  imports: [CommonModule, MatIconModule, MatButtonModule, MatSlideToggleModule, MatTooltipModule, TranslateModule],
  template: `
    <!-- ===== DASHBOARD SECTION ===== -->
    <section class="dashboard-section">
      <div class="dashboard-container">

        <!-- Section header -->
        <div class="dashboard-header">
          <div class="header-title-row">
            <span class="live-dot green"></span>
            <h2 class="dashboard-title">{{ 'landing.dashboard.title' | translate }}</h2>
            <!-- Sound toggle -->
            <div class="sound-toggle-wrap"
                 [matTooltip]="soundEnabled() ? ('landing.dashboard.sound_on' | translate) : ('landing.dashboard.sound_off' | translate)">
              <mat-icon class="sound-icon">{{ soundEnabled() ? 'volume_up' : 'volume_off' }}</mat-icon>
              <mat-slide-toggle
                [checked]="soundEnabled()"
                (change)="toggleSound($event.checked)"
                color="primary"
                class="sound-toggle">
              </mat-slide-toggle>
            </div>
          </div>
          <p class="dashboard-subtitle">{{ 'landing.dashboard.subtitle_text' | translate }}</p>
        </div>

        <!-- Search bar -->
        <div class="search-bar">
          <mat-icon class="search-icon">search</mat-icon>
          <input type="text"
                 class="search-input"
                 [placeholder]="'landing.dashboard.search_placeholder' | translate"
                 (input)="onSearch($event)"
                 [value]="searchQuery()">
          @if (searchQuery()) {
            <button class="search-clear-btn" (click)="clearSearch()"
                    [matTooltip]="'landing.dashboard.search_clear' | translate">
              <mat-icon>close</mat-icon>
            </button>
          }
        </div>

        <!-- Extended filter bar -->
        <div class="filter-bar">
          <div class="filter-row">
            <mat-icon class="filter-icon">filter_list</mat-icon>

            <!-- Region/Ville dropdown -->
            <select
              class="region-select"
              [value]="selectedRegion()"
              (change)="onRegionChange($event)">
              <option value="">{{ 'landing.dashboard.all_regions' | translate }}</option>
              @for (region of regions; track region) {
                <option [value]="region">{{ region }}</option>
              }
            </select>

            <!-- Ville dropdown -->
            <select
              class="region-select"
              [value]="selectedVille()"
              (change)="onVilleChange($event)">
              <option value="">{{ 'landing.dashboard.filter_ville' | translate }}</option>
              @for (ville of villes; track ville) {
                <option [value]="ville">{{ ville }}</option>
              }
            </select>

            <!-- Race filter -->
            <select
              class="region-select"
              [value]="selectedRace()"
              (change)="onRaceChange($event)">
              <option value="">{{ 'landing.dashboard.filter_race' | translate }}</option>
              @for (race of racesFilter; track race) {
                <option [value]="race">{{ race }}</option>
              }
            </select>

            <!-- Sort by quantity -->
            <select
              class="region-select region-select-short"
              [value]="sortQuantity()"
              (change)="onSortQuantityChange($event)">
              <option value="">{{ 'landing.dashboard.sort_quantity' | translate }}</option>
              <option value="asc">Quantite &#8593;</option>
              <option value="desc">Quantite &#8595;</option>
            </select>

            <span class="auto-refresh-indicator">
              <mat-icon class="refresh-spin">sync</mat-icon>
              <span class="refresh-text">{{ 'landing.dashboard.updated_ago' | translate }} {{ secondsSinceUpdate() }}{{ 'landing.dashboard.seconds' | translate }}</span>
            </span>
          </div>

          <!-- Role tabs -->
          <div class="role-tabs">
            <button class="role-tab" [class.role-tab-active]="activeRoleTab() === 'all'" (click)="setRoleTab('all')">
              {{ 'landing.dashboard.all_roles' | translate }}
            </button>
            <button class="role-tab role-tab-green" [class.role-tab-active]="activeRoleTab() === 'eleveurs'" (click)="setRoleTab('eleveurs')">
              <mat-icon class="role-tab-icon">agriculture</mat-icon>
              {{ 'landing.dashboard.eleveurs_tab' | translate }}
            </button>
            <button class="role-tab role-tab-blue" [class.role-tab-active]="activeRoleTab() === 'clients'" (click)="setRoleTab('clients')">
              <mat-icon class="role-tab-icon">shopping_cart</mat-icon>
              {{ 'landing.dashboard.clients_tab' | translate }}
            </button>
            <button class="role-tab role-tab-gold" [class.role-tab-active]="activeRoleTab() === 'producteurs'" (click)="setRoleTab('producteurs')">
              <mat-icon class="role-tab-icon">inventory_2</mat-icon>
              {{ 'landing.dashboard.producteurs_tab' | translate }}
            </button>
            <button class="role-tab role-tab-orange" [class.role-tab-active]="activeRoleTab() === 'poussins'" (click)="setRoleTab('poussins')">
              <mat-icon class="role-tab-icon">egg</mat-icon>
              {{ 'landing.dashboard.poussins_tab' | translate }}
            </button>
          </div>
        </div>

        <!-- 2x2 board grid -->
        <div class="boards-grid" [class.boards-single]="activeRoleTab() !== 'all'">

          <!-- ========== OFFRES ELEVEURS ========== -->
          @if (activeRoleTab() === 'all' || activeRoleTab() === 'eleveurs') {
          <div class="board board-offres">
            <div class="board-header board-header-green">
              <mat-icon>agriculture</mat-icon>
              <span class="board-header-title">{{ 'landing.dashboard.offres_title' | translate }}</span>
              <span class="board-count">{{ filteredAnnonces().length }}</span>
            </div>
            <div class="board-columns offres-columns">
              <span>{{ 'landing.dashboard.race' | translate }}</span>
              <span>{{ 'landing.dashboard.quantity' | translate }}</span>
              <span class="hide-mobile">{{ 'landing.dashboard.weight' | translate }}</span>
              <span>{{ 'landing.dashboard.price_kg' | translate }}</span>
              <span class="hide-tablet">{{ 'landing.dashboard.location' | translate }}</span>
              <span class="hide-tablet">{{ 'landing.dashboard.available' | translate }}</span>
              <span></span>
            </div>
            <div class="board-body" #offresBody>
              <div class="board-scroll-inner"
                   [style.transform]="'translateY(-' + offresScrollOffset() + 'px)'"
                   [style.transition]="offresTransition()">
                @for (annonce of displayedAnnonces(); track annonce.id) {
                  <div class="board-row offres-row role-border-green"
                       [class.row-new]="isNew(annonce.createdAt)"
                       [class.row-flash]="isNew(annonce.createdAt)"
                       [class.row-glow-new]="isVeryNew(annonce.createdAt)">
                    <span class="cell cell-race">
                      <span class="role-icon-badge badge-eleveur" matTooltip="Eleveur">
                        <mat-icon>agriculture</mat-icon>
                      </span>
                      <span class="race-dot" [style.background]="annonce.raceColor"></span>
                      <span class="race-name">{{ annonce.race }}</span>
                      @if (isVeryNew(annonce.createdAt)) {
                        <span class="badge-new badge-new-enhanced">{{ 'landing.dashboard.new' | translate }}</span>
                      } @else if (isNew(annonce.createdAt)) {
                        <span class="badge-new">{{ 'landing.dashboard.new' | translate }}</span>
                      }
                    </span>
                    <span class="cell cell-mono">{{ annonce.quantity }}</span>
                    <span class="cell cell-mono hide-mobile">{{ annonce.weightKg }} kg</span>
                    <span class="cell cell-mono cell-price">{{ formatNumber(annonce.pricePerKg) }}</span>
                    <span class="cell hide-tablet">{{ annonce.location }}</span>
                    <span class="cell hide-tablet">
                      <span class="status-dot"
                            [class.status-available]="annonce.status === 'available'"
                            [class.status-partial]="annonce.status === 'partial'"
                            [class.status-sold]="annonce.status === 'sold'"></span>
                      {{ getStatusLabel(annonce.status) }}
                    </span>
                    <span class="cell cell-actions">
                      <button class="btn-board btn-details"
                              [matTooltip]="isLoggedIn() ? '' : ('landing.dashboard.login_to_access' | translate)"
                              (click)="openDetails('annonce', annonce)">
                        <mat-icon>visibility</mat-icon>
                      </button>
                      <button class="btn-board btn-contact"
                              [matTooltip]="isLoggedIn() ? '' : ('landing.dashboard.login_to_access' | translate)"
                              (click)="contactUser('annonce', annonce)">
                        <mat-icon>chat</mat-icon>
                      </button>
                    </span>
                  </div>
                }
              </div>
            </div>
            @if (annoncesDisplayCount() < filteredAnnonces().length) {
              <button class="btn-load-more" (click)="loadMoreAnnonces()">
                <mat-icon>expand_more</mat-icon>
                {{ 'landing.dashboard.load_more' | translate }}
              </button>
            }
          </div>
          }

          <!-- ========== DEMANDES CLIENTS ========== -->
          @if (activeRoleTab() === 'all' || activeRoleTab() === 'clients') {
          <div class="board board-demandes">
            <div class="board-header board-header-blue">
              <mat-icon>shopping_cart</mat-icon>
              <span class="board-header-title">{{ 'landing.dashboard.demandes_title' | translate }}</span>
              <span class="board-count">{{ filteredBesoins().length }}</span>
            </div>
            <div class="board-columns demandes-columns">
              <span>{{ 'landing.dashboard.type' | translate }}</span>
              <span>{{ 'landing.dashboard.race' | translate }}</span>
              <span>{{ 'landing.dashboard.quantity' | translate }}</span>
              <span>{{ 'landing.dashboard.budget' | translate }}</span>
              <span class="hide-tablet">{{ 'landing.dashboard.location' | translate }}</span>
              <span class="hide-tablet">{{ 'landing.dashboard.frequency' | translate }}</span>
              <span></span>
            </div>
            <div class="board-body" #demandesBody>
              <div class="board-scroll-inner"
                   [style.transform]="'translateY(-' + demandesScrollOffset() + 'px)'"
                   [style.transition]="demandesTransition()">
                @for (besoin of displayedBesoins(); track besoin.id) {
                  <div class="board-row demandes-row role-border-blue"
                       [class.row-new]="isNew(besoin.createdAt)"
                       [class.row-flash]="isNew(besoin.createdAt)"
                       [class.row-glow-new]="isVeryNew(besoin.createdAt)">
                    <span class="cell cell-type">
                      <span class="role-icon-badge badge-client" matTooltip="Client">
                        <mat-icon>shopping_cart</mat-icon>
                      </span>
                      <mat-icon class="type-icon">{{ getTypeIcon(besoin.type) }}</mat-icon>
                    </span>
                    <span class="cell cell-race">
                      <span class="race-name">{{ besoin.race }}</span>
                      @if (isVeryNew(besoin.createdAt)) {
                        <span class="badge-new badge-new-enhanced">{{ 'landing.dashboard.new' | translate }}</span>
                      } @else if (isNew(besoin.createdAt)) {
                        <span class="badge-new">{{ 'landing.dashboard.new' | translate }}</span>
                      }
                    </span>
                    <span class="cell cell-mono">{{ besoin.quantity }}</span>
                    <span class="cell cell-mono cell-price">{{ formatNumber(besoin.budgetPerKg) }}</span>
                    <span class="cell hide-tablet">{{ besoin.location }}</span>
                    <span class="cell hide-tablet">
                      <span class="freq-badge"
                            [class.freq-hebdo]="besoin.frequency === 'hebdo'"
                            [class.freq-mensuel]="besoin.frequency === 'mensuel'"
                            [class.freq-ponctuel]="besoin.frequency === 'ponctuel'">
                        {{ getFreqLabel(besoin.frequency) }}
                      </span>
                    </span>
                    <span class="cell cell-actions">
                      <button class="btn-board btn-details"
                              [matTooltip]="isLoggedIn() ? '' : ('landing.dashboard.login_to_access' | translate)"
                              (click)="openDetails('besoin', besoin)">
                        <mat-icon>visibility</mat-icon>
                      </button>
                      <button class="btn-board btn-contact"
                              [matTooltip]="isLoggedIn() ? '' : ('landing.dashboard.login_to_access' | translate)"
                              (click)="contactUser('besoin', besoin)">
                        <mat-icon>chat</mat-icon>
                      </button>
                    </span>
                  </div>
                }
              </div>
            </div>
            @if (besoinsDisplayCount() < filteredBesoins().length) {
              <button class="btn-load-more" (click)="loadMoreBesoins()">
                <mat-icon>expand_more</mat-icon>
                {{ 'landing.dashboard.load_more' | translate }}
              </button>
            }
          </div>
          }

          <!-- ========== ALIMENTS DISPONIBLES ========== -->
          @if (activeRoleTab() === 'all' || activeRoleTab() === 'producteurs') {
          <div class="board board-aliments">
            <div class="board-header board-header-gold">
              <mat-icon>inventory_2</mat-icon>
              <span class="board-header-title">{{ 'landing.dashboard.aliments_title' | translate }}</span>
              <span class="board-count board-count-dark">{{ filteredAliments().length }}</span>
            </div>
            <div class="board-columns aliments-columns">
              <span>{{ 'landing.dashboard.product' | translate }}</span>
              <span>{{ 'landing.dashboard.price_sac' | translate }}</span>
              <span>{{ 'landing.dashboard.stock' | translate }}</span>
              <span class="hide-tablet">{{ 'landing.dashboard.zone' | translate }}</span>
              <span></span>
            </div>
            <div class="board-body board-body-short" #alimentsBody>
              <div class="board-scroll-inner"
                   [style.transform]="'translateY(-' + alimentsScrollOffset() + 'px)'"
                   [style.transition]="alimentsTransition()">
                @for (aliment of displayedAliments(); track aliment.id) {
                  <div class="board-row aliments-row role-border-gold"
                       [class.row-new]="isNew(aliment.createdAt)"
                       [class.row-glow-new]="isVeryNew(aliment.createdAt)">
                    <span class="cell">
                      <span class="role-icon-badge badge-producteur" matTooltip="Producteur">
                        <mat-icon>inventory_2</mat-icon>
                      </span>
                      {{ aliment.product }}
                      @if (isVeryNew(aliment.createdAt)) {
                        <span class="badge-new badge-new-enhanced">{{ 'landing.dashboard.new' | translate }}</span>
                      } @else if (isNew(aliment.createdAt)) {
                        <span class="badge-new">{{ 'landing.dashboard.new' | translate }}</span>
                      }
                    </span>
                    <span class="cell cell-mono cell-price">{{ formatNumber(aliment.pricePerSac) }}</span>
                    <span class="cell cell-mono">
                      <span class="stock-bar">
                        <span class="stock-fill" [style.width.%]="getStockPercent(aliment.stock)"></span>
                      </span>
                      {{ aliment.stock }}
                    </span>
                    <span class="cell hide-tablet">{{ aliment.zone }}</span>
                    <span class="cell cell-actions">
                      <button class="btn-board btn-details"
                              [matTooltip]="isLoggedIn() ? '' : ('landing.dashboard.login_to_access' | translate)"
                              (click)="openDetails('aliment', aliment)">
                        <mat-icon>visibility</mat-icon>
                      </button>
                      <button class="btn-board btn-contact"
                              [matTooltip]="isLoggedIn() ? '' : ('landing.dashboard.login_to_access' | translate)"
                              (click)="contactUser('aliment', aliment)">
                        <mat-icon>chat</mat-icon>
                      </button>
                    </span>
                  </div>
                }
              </div>
            </div>
            @if (alimentsDisplayCount() < filteredAliments().length) {
              <button class="btn-load-more" (click)="loadMoreAliments()">
                <mat-icon>expand_more</mat-icon>
                {{ 'landing.dashboard.load_more' | translate }}
              </button>
            }
          </div>
          }

          <!-- ========== POUSSINS DISPONIBLES ========== -->
          @if (activeRoleTab() === 'all' || activeRoleTab() === 'poussins') {
          <div class="board board-poussins">
            <div class="board-header board-header-orange">
              <mat-icon>egg</mat-icon>
              <span class="board-header-title">{{ 'landing.dashboard.poussins_title' | translate }}</span>
              <span class="board-count">{{ filteredPoussins().length }}</span>
            </div>
            <div class="board-columns poussins-columns">
              <span>{{ 'landing.dashboard.race' | translate }}</span>
              <span>{{ 'landing.dashboard.age_days' | translate }}</span>
              <span>{{ 'landing.dashboard.quantity' | translate }}</span>
              <span>{{ 'landing.dashboard.price_unit' | translate }}</span>
              <span class="hide-tablet">{{ 'landing.dashboard.vaccinated' | translate }}</span>
              <span class="hide-tablet">{{ 'landing.dashboard.producer' | translate }}</span>
              <span class="hide-mobile">{{ 'landing.dashboard.location' | translate }}</span>
              <span></span>
            </div>
            <div class="board-body board-body-short" #poussinsBody>
              <div class="board-scroll-inner"
                   [style.transform]="'translateY(-' + poussinsScrollOffset() + 'px)'"
                   [style.transition]="poussinsTransition()">
                @for (poussin of displayedPoussins(); track poussin.id) {
                  <div class="board-row poussins-row role-border-orange"
                       [class.row-new]="isNew(poussin.createdAt)"
                       [class.row-glow-new]="isVeryNew(poussin.createdAt)">
                    <span class="cell cell-race">
                      <span class="role-icon-badge badge-poussin" matTooltip="Poussin">
                        <mat-icon>egg</mat-icon>
                      </span>
                      <span class="race-name">{{ poussin.race }}</span>
                      @if (isVeryNew(poussin.createdAt)) {
                        <span class="badge-new badge-new-enhanced">{{ 'landing.dashboard.new' | translate }}</span>
                      } @else if (isNew(poussin.createdAt)) {
                        <span class="badge-new">{{ 'landing.dashboard.new' | translate }}</span>
                      }
                    </span>
                    <span class="cell cell-mono">{{ poussin.ageJours }} {{ poussin.ageJours <= 1 ? ('landing.dashboard.day' | translate) : ('landing.dashboard.days' | translate) }}</span>
                    <span class="cell cell-mono">{{ poussin.quantity }}</span>
                    <span class="cell cell-mono cell-price">{{ formatNumber(poussin.priceUnit) }}</span>
                    <span class="cell hide-tablet">
                      @if (poussin.vaccinated) {
                        <mat-icon class="vaccinated-yes">check_circle</mat-icon>
                      } @else {
                        <mat-icon class="vaccinated-no">cancel</mat-icon>
                      }
                    </span>
                    <span class="cell hide-tablet">{{ poussin.producteur }}</span>
                    <span class="cell hide-mobile">{{ poussin.location }}</span>
                    <span class="cell cell-actions">
                      <button class="btn-board btn-details"
                              [matTooltip]="isLoggedIn() ? '' : ('landing.dashboard.login_to_access' | translate)"
                              (click)="openDetails('poussin', poussin)">
                        <mat-icon>visibility</mat-icon>
                      </button>
                      <button class="btn-board btn-contact"
                              [matTooltip]="isLoggedIn() ? '' : ('landing.dashboard.login_to_access' | translate)"
                              (click)="contactUser('poussin', poussin)">
                        <mat-icon>chat</mat-icon>
                      </button>
                    </span>
                  </div>
                }
              </div>
            </div>
            @if (poussinsDisplayCount() < filteredPoussins().length) {
              <button class="btn-load-more" (click)="loadMorePoussins()">
                <mat-icon>expand_more</mat-icon>
                {{ 'landing.dashboard.load_more' | translate }}
              </button>
            }
          </div>
          }
        </div>

        <!-- ========== CONTRATS EN DIRECT ========== -->
        <div class="contracts-live-section">
          <div class="contracts-live-header">
            <span class="live-dot green"></span>
            <h3 class="contracts-live-title">{{ 'landing.dashboard.contracts_live' | translate }}</h3>
          </div>

          <!-- Validated contract cards -->
          <div class="validated-contracts-area">
            @for (contract of visibleContracts(); track contract.id) {
              <div class="contract-card contract-card-slide-in">
                <div class="contract-card-confetti"></div>
                <div class="contract-card-inner">
                  <span class="contract-check-pulse">
                    <mat-icon>check_circle</mat-icon>
                  </span>
                  <div class="contract-card-text">
                    <strong>{{ 'landing.dashboard.contract_validated' | translate }}</strong>
                    <span class="contract-detail-line">
                      {{ contract.eleveurName }} &#8596; {{ contract.clientName }}
                      &mdash; {{ contract.quantity }} {{ contract.race }}/{{ contract.frequency === 'hebdo' ? 'semaine' : contract.frequency }}
                    </span>
                  </div>
                </div>
              </div>
            }
          </div>

          <!-- Upcoming deliveries ticker -->
          @if (upcomingDeliveries().length > 0) {
          <div class="ticker-container">
            <div class="ticker-track">
              @for (delivery of tickerDeliveries(); track delivery.id + $index) {
                <span class="ticker-item">
                  @if (delivery.daysLeft <= 3) {
                    <span class="urgent-badge">{{ 'landing.dashboard.urgent' | translate }}</span>
                  }
                  <span class="ticker-warning">&#9888;&#65039;</span>
                  {{ 'landing.dashboard.delivery_in_days' | translate : { days: delivery.daysLeft } }}:
                  {{ delivery.eleveurName }} &#8594; {{ delivery.clientName }}
                  &mdash; {{ delivery.quantity }} {{ delivery.race }}
                  <span class="ticker-separator">|||</span>
                </span>
              }
            </div>
          </div>
          }
        </div>

        <!-- Bottom stats bar -->
        <div class="stats-bar">
          <div class="stat-pill stat-green">
            <span class="pulse-dot green"></span>
            <span class="stat-value" [attr.data-target]="stats().activeMatchings">{{ animatedMatchings() }}</span>
            <span class="stat-label-text">{{ 'landing.dashboard.matchings' | translate }}</span>
          </div>
          <div class="stat-pill stat-blue">
            <span class="pulse-dot blue"></span>
            <span class="stat-value" [attr.data-target]="stats().liveUsers">{{ animatedLiveUsers() }}</span>
            <span class="stat-label-text">{{ 'landing.dashboard.live_users' | translate }}</span>
          </div>
          <div class="stat-pill stat-contracts-today">
            <mat-icon class="stat-pill-icon stat-pill-icon-green">verified</mat-icon>
            <span class="stat-value stat-value-green">{{ animatedContractsToday() }}</span>
            <span class="stat-label-text">{{ 'landing.dashboard.contracts_today' | translate }}</span>
          </div>
          <div class="stat-pill stat-new-besoins">
            <mat-icon class="stat-pill-icon stat-pill-icon-blue">fiber_new</mat-icon>
            <span class="stat-value stat-value-blue">{{ animatedNewBesoins() }}</span>
            <span class="stat-label-text">{{ 'landing.dashboard.new_besoins' | translate }}</span>
          </div>
        </div>

      </div>
    </section>

    <!-- ========== DETAIL SLIDE-OVER ========== -->
    @if (detailOpen()) {
      <div class="overlay" (click)="closeDetails()"></div>
      <div class="slide-over" [class.slide-over-open]="detailOpen()">
        <div class="slide-over-header">
          <h3>{{ detailTitle() }}</h3>
          <button class="slide-close" (click)="closeDetails()">
            <mat-icon>close</mat-icon>
          </button>
        </div>
        <div class="slide-over-body">
          @if (detailType() === 'annonce' && detailAnnonce()) {
            <div class="detail-section">
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.race' | translate }}</span>
                <span class="detail-value">
                  <span class="race-dot" [style.background]="detailAnnonce()!.raceColor"></span>
                  {{ detailAnnonce()!.race }}
                </span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.quantity' | translate }}</span>
                <span class="detail-value">{{ detailAnnonce()!.quantity }} {{ 'landing.dashboard.heads' | translate }}</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.weight' | translate }}</span>
                <span class="detail-value">{{ detailAnnonce()!.weightKg }} kg</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.price_kg' | translate }}</span>
                <span class="detail-value detail-price">{{ formatNumber(detailAnnonce()!.pricePerKg) }} FCFA</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.location' | translate }}</span>
                <span class="detail-value">{{ detailAnnonce()!.location }}</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.available' | translate }}</span>
                <span class="detail-value">{{ detailAnnonce()!.availability }}</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.status_label' | translate }}</span>
                <span class="detail-value">
                  <span class="status-dot"
                        [class.status-available]="detailAnnonce()!.status === 'available'"
                        [class.status-partial]="detailAnnonce()!.status === 'partial'"
                        [class.status-sold]="detailAnnonce()!.status === 'sold'"></span>
                  {{ getStatusLabel(detailAnnonce()!.status) }}
                </span>
              </div>
            </div>
            <div class="detail-description">
              <p>{{ detailAnnonce()!.description }}</p>
            </div>
            <div class="detail-seller">
              <div class="seller-info">
                <mat-icon>person</mat-icon>
                <span>{{ detailAnnonce()!.eleveurName }}</span>
                @if (detailAnnonce()!.rating) {
                  <span class="seller-rating">
                    <mat-icon class="star-icon">star</mat-icon>
                    {{ detailAnnonce()!.rating }}
                  </span>
                }
              </div>
              <div class="seller-phone">
                <mat-icon>phone</mat-icon>
                <span>{{ detailAnnonce()!.phone }}</span>
              </div>
              <p class="login-hint">{{ 'landing.dashboard.login_to_contact' | translate }}</p>
            </div>
          }

          @if (detailType() === 'besoin' && detailBesoin()) {
            <div class="detail-section">
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.type' | translate }}</span>
                <span class="detail-value">
                  <mat-icon class="type-icon">{{ getTypeIcon(detailBesoin()!.type) }}</mat-icon>
                  {{ getTypeLabel(detailBesoin()!.type) }}
                </span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.race' | translate }}</span>
                <span class="detail-value">{{ detailBesoin()!.race }}</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.quantity' | translate }}</span>
                <span class="detail-value">{{ detailBesoin()!.quantity }} {{ 'landing.dashboard.heads' | translate }}</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.min_weight' | translate }}</span>
                <span class="detail-value">{{ detailBesoin()!.minWeightKg }} kg</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.budget' | translate }}</span>
                <span class="detail-value detail-price">{{ formatNumber(detailBesoin()!.budgetPerKg) }} FCFA/kg</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.location' | translate }}</span>
                <span class="detail-value">{{ detailBesoin()!.location }}</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.date' | translate }}</span>
                <span class="detail-value">{{ detailBesoin()!.date }}</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.frequency' | translate }}</span>
                <span class="detail-value">
                  <span class="freq-badge"
                        [class.freq-hebdo]="detailBesoin()!.frequency === 'hebdo'"
                        [class.freq-mensuel]="detailBesoin()!.frequency === 'mensuel'"
                        [class.freq-ponctuel]="detailBesoin()!.frequency === 'ponctuel'">
                    {{ getFreqLabel(detailBesoin()!.frequency) }}
                  </span>
                </span>
              </div>
            </div>
            <div class="detail-description">
              <p>{{ detailBesoin()!.description }}</p>
            </div>
            <div class="detail-seller">
              <div class="seller-info">
                <mat-icon>person</mat-icon>
                <span>{{ detailBesoin()!.clientName }}</span>
                @if (detailBesoin()!.rating) {
                  <span class="seller-rating">
                    <mat-icon class="star-icon">star</mat-icon>
                    {{ detailBesoin()!.rating }}
                  </span>
                }
              </div>
              <div class="seller-phone">
                <mat-icon>phone</mat-icon>
                <span>{{ detailBesoin()!.phone }}</span>
              </div>
              <p class="login-hint">{{ 'landing.dashboard.login_to_contact' | translate }}</p>
            </div>
          }

          @if (detailType() === 'aliment' && detailAliment()) {
            <div class="detail-section">
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.product' | translate }}</span>
                <span class="detail-value">{{ detailAliment()!.product }}</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.price_sac' | translate }}</span>
                <span class="detail-value detail-price">{{ formatNumber(detailAliment()!.pricePerSac) }} FCFA</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.stock' | translate }}</span>
                <span class="detail-value">{{ detailAliment()!.stock }} {{ 'landing.dashboard.sacs' | translate }}</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.zone' | translate }}</span>
                <span class="detail-value">{{ detailAliment()!.zone }}</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.target_race' | translate }}</span>
                <span class="detail-value">{{ detailAliment()!.targetRace }}</span>
              </div>
            </div>
            <div class="detail-description">
              <p>{{ detailAliment()!.description }}</p>
            </div>
            <div class="detail-seller">
              <div class="seller-info">
                <mat-icon>business</mat-icon>
                <span>{{ detailAliment()!.producteurName }}</span>
                @if (detailAliment()!.rating) {
                  <span class="seller-rating">
                    <mat-icon class="star-icon">star</mat-icon>
                    {{ detailAliment()!.rating }}
                  </span>
                }
              </div>
              <div class="seller-phone">
                <mat-icon>phone</mat-icon>
                <span>{{ detailAliment()!.phone }}</span>
              </div>
              <p class="login-hint">{{ 'landing.dashboard.login_to_contact' | translate }}</p>
            </div>
          }

          @if (detailType() === 'poussin' && detailPoussin()) {
            <div class="detail-section">
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.race' | translate }}</span>
                <span class="detail-value">{{ detailPoussin()!.race }}</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.age_days' | translate }}</span>
                <span class="detail-value">{{ detailPoussin()!.ageJours }} {{ detailPoussin()!.ageJours <= 1 ? ('landing.dashboard.day' | translate) : ('landing.dashboard.days' | translate) }}</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.quantity' | translate }}</span>
                <span class="detail-value">{{ formatNumber(detailPoussin()!.quantity) }}</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.price_unit' | translate }}</span>
                <span class="detail-value detail-price">{{ formatNumber(detailPoussin()!.priceUnit) }} FCFA</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.vaccinated' | translate }}</span>
                <span class="detail-value">
                  @if (detailPoussin()!.vaccinated) {
                    <mat-icon class="vaccinated-yes">check_circle</mat-icon>
                    {{ 'landing.dashboard.yes' | translate }}
                  } @else {
                    <mat-icon class="vaccinated-no">cancel</mat-icon>
                    {{ 'landing.dashboard.no' | translate }}
                  }
                </span>
              </div>
              @if (detailPoussin()!.vaccinationDetails) {
                <div class="detail-row">
                  <span class="detail-label">Vaccins</span>
                  <span class="detail-value">{{ detailPoussin()!.vaccinationDetails }}</span>
                </div>
              }
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.location' | translate }}</span>
                <span class="detail-value">{{ detailPoussin()!.location }}</span>
              </div>
              <div class="detail-row">
                <span class="detail-label">{{ 'landing.dashboard.available' | translate }}</span>
                <span class="detail-value">{{ detailPoussin()!.availableFrom }}</span>
              </div>
            </div>
            <div class="detail-seller">
              <div class="seller-info">
                <mat-icon>business</mat-icon>
                <span>{{ detailPoussin()!.producteur }}</span>
              </div>
              <p class="login-hint">{{ 'landing.dashboard.login_to_contact' | translate }}</p>
            </div>
          }

          <button class="btn-contact-full" (click)="contactFromDetail()">
            <mat-icon>chat</mat-icon>
            {{ 'landing.dashboard.contact' | translate }}
          </button>
        </div>
      </div>
    }
  `,
  styles: [`
    /* ================================================================
       HOST & VARIABLES
       ================================================================ */
    :host {
      display: block;
      --green: #009639;
      --green-dark: #006B28;
      --gold: #FCD116;
      --gold-dark: #D4A800;
      --dark: #1B3A5C;
      --dark-deep: #0F1B2D;
      --dark-row: #0D1926;
      --dark-row-alt: #111F33;
      --board-border: rgba(255, 255, 255, 0.06);
      --text-light: #E8ECF1;
      --text-dim: #8899AA;
      --blue: #1B3A5C;
      --blue-bright: #2196F3;
      --red: #EF2B2D;
      --orange: #FF9800;
      --radius: 12px;
      --radius-sm: 6px;
      --transition: 0.3s cubic-bezier(0.4, 0, 0.2, 1);
      --mono: 'JetBrains Mono', 'Fira Code', 'Source Code Pro', 'Courier New', monospace;
      --row-h: 48px;
    }

    /* ================================================================
       DASHBOARD SECTION
       ================================================================ */
    .dashboard-section {
      background: linear-gradient(180deg, #F8FAFC 0%, #EDF1F7 100%);
      padding: 64px 24px 72px;
    }

    .dashboard-container {
      max-width: 1400px;
      margin: 0 auto;
    }

    /* ================================================================
       HEADER
       ================================================================ */
    .dashboard-header {
      text-align: center;
      margin-bottom: 32px;
    }

    .header-title-row {
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 12px;
      margin-bottom: 8px;
    }

    .live-dot {
      width: 12px;
      height: 12px;
      border-radius: 50%;
      flex-shrink: 0;
    }

    .live-dot.green {
      background: var(--green);
      box-shadow: 0 0 8px rgba(0, 150, 57, 0.6);
      animation: pulse-glow 2s ease-in-out infinite;
    }

    @keyframes pulse-glow {
      0%, 100% { box-shadow: 0 0 6px rgba(0, 150, 57, 0.4); }
      50% { box-shadow: 0 0 16px rgba(0, 150, 57, 0.8); }
    }

    .dashboard-title {
      font-size: 2rem;
      font-weight: 800;
      color: #1E293B;
      margin: 0;
      letter-spacing: -0.3px;
    }

    .dashboard-subtitle {
      font-size: 1rem;
      color: #64748B;
      margin: 0;
    }

    /* Sound toggle */
    .sound-toggle-wrap {
      display: flex;
      align-items: center;
      gap: 6px;
      margin-left: 16px;
      cursor: pointer;
    }

    .sound-icon {
      font-size: 20px;
      width: 20px;
      height: 20px;
      color: #64748B;
    }

    .sound-toggle {
      transform: scale(0.75);
    }

    /* ================================================================
       SEARCH BAR
       ================================================================ */
    .search-bar {
      display: flex;
      align-items: center;
      gap: 10px;
      background: #1a2740;
      border: 1px solid rgba(255, 255, 255, 0.08);
      border-radius: 10px;
      padding: 10px 18px;
      margin-bottom: 12px;
      transition: border-color 0.3s ease, box-shadow 0.3s ease;
    }

    .search-bar:focus-within {
      border-color: var(--green);
      box-shadow: 0 0 12px rgba(0, 150, 57, 0.15);
    }

    .search-icon {
      color: var(--text-dim);
      font-size: 22px;
      width: 22px;
      height: 22px;
      flex-shrink: 0;
    }

    .search-input {
      flex: 1;
      background: transparent;
      border: none;
      outline: none;
      color: var(--text-light);
      font-size: 0.92rem;
      font-family: inherit;
      letter-spacing: 0.2px;
    }

    .search-input::placeholder {
      color: var(--text-dim);
      opacity: 0.7;
    }

    .search-clear-btn {
      display: flex;
      align-items: center;
      justify-content: center;
      width: 28px;
      height: 28px;
      border: none;
      border-radius: 50%;
      background: rgba(255, 255, 255, 0.08);
      color: var(--text-dim);
      cursor: pointer;
      transition: all 0.2s ease;
      flex-shrink: 0;
    }

    .search-clear-btn:hover {
      background: rgba(255, 255, 255, 0.15);
      color: var(--text-light);
    }

    .search-clear-btn mat-icon {
      font-size: 16px;
      width: 16px;
      height: 16px;
    }

    /* ================================================================
       FILTER BAR
       ================================================================ */
    .filter-bar {
      margin-bottom: 24px;
    }

    .filter-row {
      display: flex;
      align-items: center;
      gap: 12px;
      background: var(--dark-deep);
      border-radius: 10px;
      padding: 10px 20px;
      flex-wrap: wrap;
    }

    .filter-icon {
      color: var(--text-dim);
      font-size: 20px;
      width: 20px;
      height: 20px;
    }

    .region-select {
      background: rgba(255, 255, 255, 0.08);
      border: 1px solid rgba(255, 255, 255, 0.12);
      border-radius: 6px;
      color: var(--text-light);
      padding: 8px 14px;
      font-size: 0.88rem;
      font-family: inherit;
      cursor: pointer;
      outline: none;
      transition: border-color var(--transition);
      min-width: 160px;
    }

    .region-select-short {
      min-width: 130px;
    }

    .region-select:hover,
    .region-select:focus {
      border-color: var(--green);
    }

    .region-select option {
      background: var(--dark-deep);
      color: var(--text-light);
    }

    .auto-refresh-indicator {
      display: flex;
      align-items: center;
      gap: 6px;
      margin-left: auto;
      color: var(--text-dim);
      font-size: 0.8rem;
    }

    .refresh-spin {
      font-size: 16px;
      width: 16px;
      height: 16px;
      animation: spin 3s linear infinite;
    }

    @keyframes spin {
      from { transform: rotate(0deg); }
      to { transform: rotate(360deg); }
    }

    .refresh-text {
      font-family: var(--mono);
      font-size: 0.78rem;
      letter-spacing: 0.2px;
    }

    /* Role tabs */
    .role-tabs {
      display: flex;
      gap: 8px;
      margin-top: 10px;
      flex-wrap: wrap;
    }

    .role-tab {
      display: flex;
      align-items: center;
      gap: 6px;
      padding: 8px 18px;
      border: 1px solid rgba(255, 255, 255, 0.12);
      border-radius: 20px;
      background: var(--dark-deep);
      color: var(--text-dim);
      font-size: 0.82rem;
      font-weight: 600;
      cursor: pointer;
      transition: all 0.2s ease;
      letter-spacing: 0.3px;
    }

    .role-tab:hover {
      background: rgba(255, 255, 255, 0.06);
      color: var(--text-light);
    }

    .role-tab-active {
      color: white !important;
      border-color: var(--green) !important;
      background: rgba(0, 150, 57, 0.2) !important;
    }

    .role-tab-green.role-tab-active {
      border-color: var(--green) !important;
      background: rgba(0, 150, 57, 0.2) !important;
    }

    .role-tab-blue.role-tab-active {
      border-color: var(--blue-bright) !important;
      background: rgba(33, 150, 243, 0.15) !important;
    }

    .role-tab-gold.role-tab-active {
      border-color: var(--gold) !important;
      background: rgba(252, 209, 22, 0.15) !important;
    }

    .role-tab-orange.role-tab-active {
      border-color: var(--orange) !important;
      background: rgba(255, 152, 0, 0.15) !important;
    }

    .role-tab-icon {
      font-size: 16px;
      width: 16px;
      height: 16px;
    }

    /* ================================================================
       BOARDS GRID
       ================================================================ */
    .boards-grid {
      display: grid;
      grid-template-columns: 1fr 1fr;
      gap: 20px;
      margin-bottom: 24px;
    }

    .boards-single {
      grid-template-columns: 1fr;
    }

    @media (max-width: 1024px) {
      .boards-grid:not(.boards-single) {
        grid-template-columns: 1fr 1fr;
      }
    }

    @media (max-width: 768px) {
      .boards-grid:not(.boards-single) {
        grid-template-columns: 1fr;
      }
    }

    /* ================================================================
       BOARD COMMON
       ================================================================ */
    .board {
      background: var(--dark-deep);
      border-radius: var(--radius);
      overflow: hidden;
      box-shadow:
        0 4px 24px rgba(0, 0, 0, 0.25),
        0 0 0 1px rgba(255, 255, 255, 0.04);
    }

    /* ---- BOARD HEADER ---- */
    .board-header {
      display: flex;
      align-items: center;
      gap: 10px;
      padding: 14px 18px;
      font-weight: 700;
      font-size: 0.92rem;
      letter-spacing: 0.8px;
      text-transform: uppercase;
      color: white;
    }

    .board-header mat-icon {
      font-size: 20px;
      width: 20px;
      height: 20px;
    }

    .board-header-green {
      background: linear-gradient(135deg, var(--green) 0%, var(--green-dark) 100%);
    }

    .board-header-blue {
      background: linear-gradient(135deg, var(--dark) 0%, #0F2440 100%);
    }

    .board-header-gold {
      background: linear-gradient(135deg, var(--gold) 0%, var(--gold-dark) 100%);
      color: var(--dark-deep);
    }

    .board-header-gold mat-icon {
      color: var(--dark-deep);
    }

    .board-header-orange {
      background: linear-gradient(135deg, #FF9800 0%, #E65100 100%);
      color: white;
    }

    .board-count {
      margin-left: auto;
      background: rgba(255, 255, 255, 0.2);
      padding: 2px 10px;
      border-radius: 12px;
      font-size: 0.78rem;
      font-family: var(--mono);
    }

    .board-count-dark {
      background: rgba(0, 0, 0, 0.15);
      color: var(--dark-deep);
    }

    /* ---- COLUMN HEADERS ---- */
    .board-columns {
      display: grid;
      padding: 8px 14px;
      font-size: 0.72rem;
      font-weight: 600;
      text-transform: uppercase;
      letter-spacing: 0.6px;
      color: var(--text-dim);
      border-bottom: 1px solid var(--board-border);
      background: rgba(255, 255, 255, 0.02);
    }

    .offres-columns {
      grid-template-columns: 2fr 0.7fr 0.9fr 1fr 1.2fr 1fr 0.8fr;
    }

    .demandes-columns {
      grid-template-columns: 0.6fr 1.5fr 0.7fr 1fr 1.2fr 1fr 0.8fr;
    }

    .aliments-columns {
      grid-template-columns: 2fr 1.2fr 1.2fr 1.2fr 0.8fr;
    }

    .poussins-columns {
      grid-template-columns: 1.5fr 1fr 0.7fr 1fr 0.7fr 1.5fr 1.2fr 0.8fr;
    }

    /* ---- BOARD BODY ---- */
    .board-body {
      height: calc(var(--row-h) * 10);
      overflow: hidden;
      position: relative;
    }

    .board-body-short {
      height: calc(var(--row-h) * 5);
    }

    .board-scroll-inner {
      will-change: transform;
    }

    /* ---- ROWS ---- */
    .board-row {
      display: grid;
      align-items: center;
      padding: 0 14px;
      height: var(--row-h);
      font-size: 0.84rem;
      color: var(--text-light);
      border-bottom: 1px solid var(--board-border);
      transition: background 0.2s ease;
    }

    .board-row:nth-child(even) {
      background: var(--dark-row-alt);
    }

    .board-row:nth-child(odd) {
      background: var(--dark-row);
    }

    .board-row:hover {
      background: rgba(255, 255, 255, 0.05);
    }

    /* Visual differentiation by role - left border */
    .role-border-green {
      border-left: 3px solid var(--green);
    }

    .role-border-blue {
      border-left: 3px solid var(--blue-bright);
    }

    .role-border-gold {
      border-left: 3px solid var(--gold);
    }

    /* Role icon badge */
    .role-icon-badge {
      display: inline-flex;
      align-items: center;
      justify-content: center;
      width: 20px;
      height: 20px;
      border-radius: 50%;
      flex-shrink: 0;
      margin-right: 4px;
    }

    .role-icon-badge mat-icon {
      font-size: 12px;
      width: 12px;
      height: 12px;
    }

    .badge-eleveur {
      background: rgba(0, 150, 57, 0.2);
      color: var(--green);
    }

    .badge-client {
      background: rgba(33, 150, 243, 0.15);
      color: var(--blue-bright);
    }

    .badge-producteur {
      background: rgba(252, 209, 22, 0.15);
      color: var(--gold);
    }

    .offres-row {
      grid-template-columns: 2fr 0.7fr 0.9fr 1fr 1.2fr 1fr 0.8fr;
    }

    .demandes-row {
      grid-template-columns: 0.6fr 1.5fr 0.7fr 1fr 1.2fr 1fr 0.8fr;
    }

    .aliments-row {
      grid-template-columns: 2fr 1.2fr 1.2fr 1.2fr 0.8fr;
    }

    .poussins-row {
      grid-template-columns: 1.5fr 1fr 0.7fr 1fr 0.7fr 1.5fr 1.2fr 0.8fr;
    }

    .role-border-orange {
      border-left: 3px solid var(--orange);
    }

    .badge-poussin {
      background: rgba(255, 152, 0, 0.2);
      color: var(--orange);
    }

    .vaccinated-yes {
      color: #4CAF50;
      font-size: 18px;
      width: 18px;
      height: 18px;
    }

    .vaccinated-no {
      color: var(--red);
      font-size: 18px;
      width: 18px;
      height: 18px;
    }

    /* ---- NEW ROW FLASH ---- */
    .row-flash {
      animation: flash-new 2s ease-out;
    }

    @keyframes flash-new {
      0% { background: rgba(252, 209, 22, 0.20); }
      50% { background: rgba(252, 209, 22, 0.08); }
      100% { background: transparent; }
    }

    /* Very new row glow (< 2 hours) */
    .row-glow-new {
      animation: row-glow 2.5s ease-in-out infinite;
    }

    @keyframes row-glow {
      0%, 100% { box-shadow: inset 0 0 6px rgba(252, 209, 22, 0.1); }
      50% { box-shadow: inset 0 0 16px rgba(252, 209, 22, 0.25); }
    }

    /* ---- CELLS ---- */
    .cell {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
      font-size: 0.82rem;
    }

    .cell-mono {
      font-family: var(--mono);
      font-size: 0.82rem;
      letter-spacing: 0.3px;
    }

    .cell-price {
      color: var(--gold);
      font-weight: 600;
    }

    .cell-race {
      display: flex;
      align-items: center;
      gap: 6px;
      overflow: hidden;
    }

    .race-dot {
      width: 8px;
      height: 8px;
      border-radius: 50%;
      flex-shrink: 0;
    }

    .race-name {
      overflow: hidden;
      text-overflow: ellipsis;
      white-space: nowrap;
    }

    .cell-type {
      display: flex;
      align-items: center;
    }

    .type-icon {
      font-size: 18px;
      width: 18px;
      height: 18px;
      color: var(--text-dim);
    }

    /* ---- BADGES ---- */
    .badge-new {
      display: inline-block;
      background: var(--gold);
      color: var(--dark-deep);
      font-size: 0.6rem;
      font-weight: 800;
      padding: 1px 5px;
      border-radius: 3px;
      letter-spacing: 0.5px;
      margin-left: 4px;
      flex-shrink: 0;
      animation: badge-pulse 1.5s ease-in-out infinite alternate;
    }

    .badge-new-enhanced {
      background: var(--gold);
      box-shadow: 0 0 8px rgba(252, 209, 22, 0.6);
      animation: badge-scale-pulse 1.2s ease-in-out infinite;
    }

    @keyframes badge-scale-pulse {
      0%, 100% { transform: scale(1); box-shadow: 0 0 6px rgba(252, 209, 22, 0.4); }
      50% { transform: scale(1.1); box-shadow: 0 0 14px rgba(252, 209, 22, 0.8); }
    }

    @keyframes badge-pulse {
      from { opacity: 0.85; }
      to { opacity: 1; }
    }

    /* Status dots */
    .status-dot {
      display: inline-block;
      width: 7px;
      height: 7px;
      border-radius: 50%;
      margin-right: 5px;
    }

    .status-available { background: #4CAF50; box-shadow: 0 0 4px rgba(76, 175, 80, 0.5); }
    .status-partial { background: var(--orange); box-shadow: 0 0 4px rgba(255, 152, 0, 0.5); }
    .status-sold { background: var(--red); box-shadow: 0 0 4px rgba(239, 43, 45, 0.5); }

    /* Frequency badges */
    .freq-badge {
      display: inline-block;
      font-size: 0.68rem;
      font-weight: 700;
      padding: 2px 8px;
      border-radius: 4px;
      letter-spacing: 0.3px;
      text-transform: uppercase;
    }

    .freq-hebdo {
      background: rgba(252, 209, 22, 0.2);
      color: var(--gold);
      border: 1px solid rgba(252, 209, 22, 0.3);
    }

    .freq-mensuel {
      background: rgba(33, 150, 243, 0.15);
      color: var(--blue-bright);
      border: 1px solid rgba(33, 150, 243, 0.25);
    }

    .freq-ponctuel {
      background: rgba(255, 255, 255, 0.08);
      color: var(--text-dim);
      border: 1px solid rgba(255, 255, 255, 0.12);
    }

    /* Stock bar */
    .stock-bar {
      display: inline-block;
      width: 36px;
      height: 5px;
      background: rgba(255, 255, 255, 0.1);
      border-radius: 3px;
      overflow: hidden;
      vertical-align: middle;
      margin-right: 6px;
    }

    .stock-fill {
      display: block;
      height: 100%;
      background: var(--green);
      border-radius: 3px;
      transition: width 0.4s ease;
    }

    /* ---- ACTION BUTTONS ---- */
    .cell-actions {
      display: flex;
      gap: 4px;
      justify-content: flex-end;
    }

    .btn-board {
      display: flex;
      align-items: center;
      justify-content: center;
      width: 30px;
      height: 30px;
      border: none;
      border-radius: 6px;
      cursor: pointer;
      transition: all 0.2s ease;
    }

    .btn-board mat-icon {
      font-size: 16px;
      width: 16px;
      height: 16px;
    }

    .btn-details {
      background: rgba(255, 255, 255, 0.06);
      color: var(--text-dim);
    }

    .btn-details:hover {
      background: rgba(255, 255, 255, 0.12);
      color: var(--text-light);
    }

    .btn-contact {
      background: rgba(0, 150, 57, 0.15);
      color: var(--green);
    }

    .btn-contact:hover {
      background: var(--green);
      color: white;
    }

    /* Load more button */
    .btn-load-more {
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 6px;
      width: 100%;
      padding: 10px;
      border: none;
      background: rgba(255, 255, 255, 0.04);
      color: var(--text-dim);
      font-size: 0.82rem;
      font-weight: 600;
      cursor: pointer;
      transition: all 0.2s ease;
      border-top: 1px solid var(--board-border);
    }

    .btn-load-more:hover {
      background: rgba(255, 255, 255, 0.08);
      color: var(--text-light);
    }

    .btn-load-more mat-icon {
      font-size: 18px;
      width: 18px;
      height: 18px;
    }

    /* ================================================================
       RESPONSIVE: HIDE COLUMNS
       ================================================================ */
    @media (max-width: 1200px) {
      .hide-tablet { display: none !important; }

      .offres-columns { grid-template-columns: 2fr 0.7fr 0.9fr 1fr 0.8fr; }
      .offres-row { grid-template-columns: 2fr 0.7fr 0.9fr 1fr 0.8fr; }

      .demandes-columns { grid-template-columns: 0.6fr 1.5fr 0.7fr 1fr 0.8fr; }
      .demandes-row { grid-template-columns: 0.6fr 1.5fr 0.7fr 1fr 0.8fr; }

      .aliments-columns { grid-template-columns: 2fr 1.2fr 1.2fr 0.8fr; }
      .aliments-row { grid-template-columns: 2fr 1.2fr 1.2fr 0.8fr; }

      .poussins-columns { grid-template-columns: 1.5fr 1fr 0.7fr 1fr 0.8fr; }
      .poussins-row { grid-template-columns: 1.5fr 1fr 0.7fr 1fr 0.8fr; }
    }

    @media (max-width: 768px) {
      .hide-mobile { display: none !important; }

      .offres-columns { grid-template-columns: 2fr 0.7fr 1fr 0.8fr; }
      .offres-row { grid-template-columns: 2fr 0.7fr 1fr 0.8fr; }

      .poussins-columns { grid-template-columns: 1.5fr 1fr 0.7fr 1fr 0.8fr; }
      .poussins-row { grid-template-columns: 1.5fr 1fr 0.7fr 1fr 0.8fr; }
    }

    /* ================================================================
       CONTRACTS LIVE SECTION
       ================================================================ */
    .contracts-live-section {
      margin-bottom: 24px;
      background: var(--dark-deep);
      border-radius: var(--radius);
      padding: 18px 20px;
      box-shadow: 0 4px 24px rgba(0, 0, 0, 0.25), 0 0 0 1px rgba(255, 255, 255, 0.04);
    }

    .contracts-live-header {
      display: flex;
      align-items: center;
      gap: 10px;
      margin-bottom: 14px;
    }

    .contracts-live-title {
      margin: 0;
      font-size: 1rem;
      font-weight: 700;
      letter-spacing: 1px;
      text-transform: uppercase;
      color: var(--text-light);
    }

    /* Validated contract cards */
    .validated-contracts-area {
      display: flex;
      flex-direction: column;
      gap: 8px;
      margin-bottom: 14px;
    }

    .contract-card {
      position: relative;
      overflow: hidden;
      border-radius: 8px;
      background: rgba(0, 150, 57, 0.08);
      border: 1px solid rgba(0, 150, 57, 0.2);
    }

    .contract-card-slide-in {
      animation: contract-slide-in 0.5s cubic-bezier(0.4, 0, 0.2, 1);
    }

    @keyframes contract-slide-in {
      from { transform: translateX(100%); opacity: 0; }
      to { transform: translateX(0); opacity: 1; }
    }

    .contract-card-confetti {
      position: absolute;
      inset: 0;
      pointer-events: none;
    }

    .contract-card-confetti::before,
    .contract-card-confetti::after {
      content: '';
      position: absolute;
      width: 6px;
      height: 6px;
      border-radius: 50%;
      animation: confetti-burst 1.5s ease-out forwards;
    }

    .contract-card-confetti::before {
      background: var(--gold);
      top: 50%;
      left: 20%;
      animation-delay: 0.1s;
    }

    .contract-card-confetti::after {
      background: var(--green);
      top: 30%;
      left: 60%;
      animation-delay: 0.3s;
    }

    @keyframes confetti-burst {
      0% { transform: scale(0) translateY(0); opacity: 1; }
      50% { transform: scale(1.5) translateY(-20px); opacity: 0.8; }
      100% { transform: scale(0.5) translateY(-40px); opacity: 0; }
    }

    .contract-card-inner {
      display: flex;
      align-items: center;
      gap: 12px;
      padding: 12px 16px;
    }

    .contract-check-pulse {
      display: flex;
      align-items: center;
      justify-content: center;
      flex-shrink: 0;
    }

    .contract-check-pulse mat-icon {
      font-size: 28px;
      width: 28px;
      height: 28px;
      color: var(--green);
      animation: check-pulse-anim 1.5s ease-in-out infinite;
    }

    @keyframes check-pulse-anim {
      0%, 100% { transform: scale(1); }
      50% { transform: scale(1.15); }
    }

    .contract-card-text {
      display: flex;
      flex-direction: column;
      gap: 2px;
    }

    .contract-card-text strong {
      color: var(--green);
      font-size: 0.88rem;
      letter-spacing: 0.3px;
    }

    .contract-detail-line {
      font-size: 0.82rem;
      color: var(--text-dim);
      font-family: var(--mono);
    }

    /* Ticker */
    .ticker-container {
      overflow: hidden;
      position: relative;
      background: rgba(239, 43, 45, 0.06);
      border: 1px solid rgba(239, 43, 45, 0.15);
      border-radius: 6px;
      padding: 8px 0;
    }

    .ticker-track {
      display: flex;
      white-space: nowrap;
      animation: ticker-scroll 30s linear infinite;
    }

    @keyframes ticker-scroll {
      0% { transform: translateX(0); }
      100% { transform: translateX(-50%); }
    }

    .ticker-item {
      display: inline-flex;
      align-items: center;
      gap: 6px;
      padding: 0 24px;
      font-size: 0.82rem;
      color: var(--text-light);
      font-family: var(--mono);
    }

    .ticker-warning {
      font-size: 1rem;
    }

    .ticker-separator {
      color: rgba(255, 255, 255, 0.15);
      margin: 0 8px;
    }

    .urgent-badge {
      display: inline-block;
      background: var(--red);
      color: white;
      font-size: 0.6rem;
      font-weight: 800;
      padding: 2px 6px;
      border-radius: 3px;
      letter-spacing: 0.5px;
      animation: blink-urgent 1s ease-in-out infinite;
    }

    @keyframes blink-urgent {
      0%, 100% { opacity: 1; }
      50% { opacity: 0.4; }
    }

    /* ================================================================
       STATS BAR
       ================================================================ */
    .stats-bar {
      display: flex;
      justify-content: center;
      gap: 24px;
      flex-wrap: wrap;
    }

    .stat-pill {
      display: flex;
      align-items: center;
      gap: 10px;
      padding: 12px 28px;
      border-radius: 50px;
      font-size: 0.9rem;
      font-weight: 600;
      background: var(--dark-deep);
      color: var(--text-light);
      box-shadow: 0 2px 12px rgba(0, 0, 0, 0.2);
    }

    .pulse-dot {
      width: 10px;
      height: 10px;
      border-radius: 50%;
      flex-shrink: 0;
    }

    .pulse-dot.green {
      background: var(--green);
      animation: pulse-dot-anim 1.5s ease-in-out infinite;
    }

    .pulse-dot.blue {
      background: var(--blue-bright);
      animation: pulse-dot-anim 1.5s ease-in-out infinite 0.3s;
    }

    @keyframes pulse-dot-anim {
      0%, 100% { opacity: 1; transform: scale(1); }
      50% { opacity: 0.6; transform: scale(0.85); }
    }

    .stat-value {
      font-family: var(--mono);
      font-size: 1.1rem;
      font-weight: 700;
      letter-spacing: 0.5px;
    }

    .stat-green .stat-value { color: var(--green); }
    .stat-blue .stat-value { color: var(--blue-bright); }
    .stat-value-green { color: var(--green); }
    .stat-value-blue { color: var(--blue-bright); }

    .stat-pill-icon {
      font-size: 20px;
      width: 20px;
      height: 20px;
    }

    .stat-pill-icon-green { color: var(--green); }
    .stat-pill-icon-blue { color: var(--blue-bright); }

    .stat-label-text {
      font-size: 0.82rem;
      color: var(--text-dim);
      font-weight: 500;
    }

    /* ================================================================
       SLIDE-OVER PANEL
       ================================================================ */
    .overlay {
      position: fixed;
      inset: 0;
      background: rgba(0, 0, 0, 0.55);
      z-index: 2000;
      animation: fade-in 0.2s ease;
    }

    @keyframes fade-in {
      from { opacity: 0; }
      to { opacity: 1; }
    }

    .slide-over {
      position: fixed;
      top: 0;
      right: 0;
      bottom: 0;
      width: 440px;
      max-width: 100vw;
      background: var(--dark-deep);
      color: var(--text-light);
      z-index: 2001;
      display: flex;
      flex-direction: column;
      box-shadow: -8px 0 40px rgba(0, 0, 0, 0.4);
      animation: slide-in 0.3s cubic-bezier(0.4, 0, 0.2, 1);
    }

    @keyframes slide-in {
      from { transform: translateX(100%); }
      to { transform: translateX(0); }
    }

    .slide-over-header {
      display: flex;
      align-items: center;
      justify-content: space-between;
      padding: 20px 24px;
      border-bottom: 1px solid var(--board-border);
    }

    .slide-over-header h3 {
      margin: 0;
      font-size: 1.1rem;
      font-weight: 700;
    }

    .slide-close {
      display: flex;
      align-items: center;
      justify-content: center;
      width: 36px;
      height: 36px;
      border: none;
      border-radius: 8px;
      background: rgba(255, 255, 255, 0.06);
      color: var(--text-dim);
      cursor: pointer;
      transition: all 0.2s ease;
    }

    .slide-close:hover {
      background: rgba(255, 255, 255, 0.12);
      color: white;
    }

    .slide-over-body {
      flex: 1;
      overflow-y: auto;
      padding: 24px;
    }

    .detail-section {
      margin-bottom: 24px;
    }

    .detail-row {
      display: flex;
      justify-content: space-between;
      align-items: center;
      padding: 10px 0;
      border-bottom: 1px solid var(--board-border);
    }

    .detail-label {
      font-size: 0.82rem;
      color: var(--text-dim);
      font-weight: 500;
      text-transform: uppercase;
      letter-spacing: 0.4px;
    }

    .detail-value {
      font-size: 0.92rem;
      font-weight: 600;
      display: flex;
      align-items: center;
      gap: 6px;
    }

    .detail-price {
      color: var(--gold);
      font-family: var(--mono);
      font-size: 1rem;
    }

    .detail-description {
      background: rgba(255, 255, 255, 0.03);
      border-radius: 8px;
      padding: 16px;
      margin-bottom: 24px;
      border: 1px solid var(--board-border);
    }

    .detail-description p {
      margin: 0;
      font-size: 0.9rem;
      color: var(--text-dim);
      line-height: 1.6;
    }

    .detail-seller {
      background: rgba(0, 150, 57, 0.06);
      border: 1px solid rgba(0, 150, 57, 0.15);
      border-radius: 10px;
      padding: 16px;
      margin-bottom: 24px;
    }

    .seller-info {
      display: flex;
      align-items: center;
      gap: 8px;
      margin-bottom: 8px;
      font-weight: 600;
    }

    .seller-info mat-icon {
      font-size: 20px;
      width: 20px;
      height: 20px;
      color: var(--green);
    }

    .seller-rating {
      display: flex;
      align-items: center;
      gap: 2px;
      margin-left: auto;
      font-size: 0.85rem;
      color: var(--gold);
    }

    .star-icon {
      font-size: 16px !important;
      width: 16px !important;
      height: 16px !important;
      color: var(--gold);
    }

    .seller-phone {
      display: flex;
      align-items: center;
      gap: 8px;
      font-family: var(--mono);
      font-size: 0.88rem;
      color: var(--text-dim);
    }

    .seller-phone mat-icon {
      font-size: 16px;
      width: 16px;
      height: 16px;
      color: var(--text-dim);
    }

    .login-hint {
      font-size: 0.78rem;
      color: var(--orange);
      margin: 10px 0 0;
      font-style: italic;
    }

    .btn-contact-full {
      display: flex;
      align-items: center;
      justify-content: center;
      gap: 8px;
      width: 100%;
      padding: 14px;
      border: none;
      border-radius: 10px;
      background: var(--green);
      color: white;
      font-size: 1rem;
      font-weight: 700;
      cursor: pointer;
      transition: all 0.2s ease;
    }

    .btn-contact-full:hover {
      background: var(--green-dark);
      transform: translateY(-1px);
      box-shadow: 0 4px 16px rgba(0, 150, 57, 0.3);
    }

    .btn-contact-full mat-icon {
      font-size: 20px;
      width: 20px;
      height: 20px;
    }

    /* ================================================================
       RESPONSIVE
       ================================================================ */
    @media (max-width: 600px) {
      .dashboard-section {
        padding: 40px 12px 48px;
      }

      .dashboard-title {
        font-size: 1.5rem;
      }

      .filter-row {
        padding: 8px 12px;
      }

      .region-select {
        min-width: 120px;
        font-size: 0.82rem;
      }

      .board-header {
        padding: 12px 14px;
        font-size: 0.8rem;
      }

      .board-columns {
        font-size: 0.65rem;
        padding: 6px 10px;
      }

      .board-row {
        padding: 0 10px;
        font-size: 0.78rem;
      }

      .cell {
        font-size: 0.76rem;
      }

      .cell-mono {
        font-size: 0.76rem;
      }

      .stat-pill {
        padding: 10px 20px;
        font-size: 0.82rem;
      }

      .slide-over {
        width: 100vw;
      }

      .stats-bar {
        gap: 12px;
      }

      .role-tabs {
        gap: 6px;
      }

      .role-tab {
        padding: 6px 12px;
        font-size: 0.75rem;
      }

      .sound-toggle-wrap {
        margin-left: 8px;
      }
    }
  `],
})
export class PublicDashboardComponent implements OnInit, AfterViewInit, OnDestroy {
  private readonly http = inject(HttpClient);
  private readonly router = inject(Router);
  private readonly platformId = inject(PLATFORM_ID);
  private readonly zone = inject(NgZone);
  private readonly el = inject(ElementRef);
  private readonly authService = inject(AuthService);
  private readonly destroy$ = new Subject<void>();

  /* ---- Data signals ---- */
  readonly annonces = signal<Annonce[]>(MOCK_ANNONCES);
  readonly besoins = signal<Besoin[]>(MOCK_BESOINS);
  readonly aliments = signal<Aliment[]>(MOCK_ALIMENTS);
  readonly poussins = signal<Poussin[]>(MOCK_POUSSINS);
  readonly stats = signal<DashboardStats>(MOCK_STATS);
  readonly validatedContracts = signal<ValidatedContract[]>(MOCK_VALIDATED_CONTRACTS);
  readonly upcomingDeliveries = signal<UpcomingDelivery[]>(MOCK_UPCOMING_DELIVERIES);

  /* ---- Search signal ---- */
  readonly searchQuery = signal('');
  private searchDebounceTimer: ReturnType<typeof setTimeout> | null = null;
  private debouncedSearchQuery = signal('');

  /* ---- Filter signals ---- */
  readonly selectedRegion = signal('');
  readonly selectedVille = signal('');
  readonly selectedRace = signal('');
  readonly sortQuantity = signal<'' | 'asc' | 'desc'>('');
  readonly activeRoleTab = signal<'all' | 'eleveurs' | 'clients' | 'producteurs' | 'poussins'>('all');
  readonly regions = REGIONS;
  readonly villes = VILLES;
  readonly racesFilter = RACES_FILTER;

  /* ---- Lazy loading signals ---- */
  readonly annoncesDisplayCount = signal(20);
  readonly besoinsDisplayCount = signal(20);
  readonly alimentsDisplayCount = signal(20);
  readonly poussinsDisplayCount = signal(20);

  /* ---- Sound ---- */
  readonly soundEnabled = signal(false);
  private notificationAudio: HTMLAudioElement | null = null;

  /* ---- Auth ---- */
  readonly isLoggedIn = this.authService.isLoggedIn;

  /* ---- Animated stats ---- */
  readonly animatedMatchings = signal(0);
  readonly animatedLiveUsers = signal(0);
  readonly animatedContractsToday = signal(0);
  readonly animatedNewBesoins = signal(0);
  private statsAnimated = false;

  /* ---- Contract cards ---- */
  readonly visibleContracts = signal<ValidatedContract[]>([]);
  private contractDismissTimers: ReturnType<typeof setTimeout>[] = [];

  /* ---- Computed filtered data ---- */
  readonly filteredAnnonces = computed(() => {
    const region = this.selectedRegion();
    const ville = this.selectedVille();
    const race = this.selectedRace();
    const sort = this.sortQuantity();
    const query = this.debouncedSearchQuery();
    let data = this.annonces();

    if (query) {
      const q = this.normalizeSearch(query);
      data = data.filter(a =>
        this.normalizeSearch(a.eleveurName).includes(q) ||
        this.normalizeSearch(a.race).includes(q) ||
        this.normalizeSearch(a.location).includes(q) ||
        (a.description && this.normalizeSearch(a.description).includes(q))
      );
    }
    if (region) {
      data = data.filter(a => a.location.toLowerCase().includes(region.toLowerCase())
        || this.regionContainsLocation(region, a.location));
    }
    if (ville) {
      data = data.filter(a => a.location.toLowerCase().includes(ville.toLowerCase()));
    }
    if (race && race !== 'Toutes') {
      data = data.filter(a => a.race.toLowerCase() === race.toLowerCase());
    }
    if (sort === 'asc') {
      data = [...data].sort((a, b) => a.quantity - b.quantity);
    } else if (sort === 'desc') {
      data = [...data].sort((a, b) => b.quantity - a.quantity);
    }
    return data;
  });

  readonly filteredBesoins = computed(() => {
    const region = this.selectedRegion();
    const ville = this.selectedVille();
    const race = this.selectedRace();
    const sort = this.sortQuantity();
    const query = this.debouncedSearchQuery();
    let data = this.besoins();

    if (query) {
      const q = this.normalizeSearch(query);
      data = data.filter(b =>
        this.normalizeSearch(b.clientName).includes(q) ||
        this.normalizeSearch(b.race).includes(q) ||
        this.normalizeSearch(b.location).includes(q) ||
        (b.description && this.normalizeSearch(b.description).includes(q))
      );
    }
    if (region) {
      data = data.filter(b => b.location.toLowerCase().includes(region.toLowerCase())
        || this.regionContainsLocation(region, b.location));
    }
    if (ville) {
      data = data.filter(b => b.location.toLowerCase().includes(ville.toLowerCase()));
    }
    if (race && race !== 'Toutes') {
      data = data.filter(b => b.race.toLowerCase() === race.toLowerCase());
    }
    if (sort === 'asc') {
      data = [...data].sort((a, b) => a.quantity - b.quantity);
    } else if (sort === 'desc') {
      data = [...data].sort((a, b) => b.quantity - a.quantity);
    }
    return data;
  });

  readonly filteredAliments = computed(() => {
    const region = this.selectedRegion();
    const ville = this.selectedVille();
    const query = this.debouncedSearchQuery();
    let data = this.aliments();

    if (query) {
      const q = this.normalizeSearch(query);
      data = data.filter(a =>
        this.normalizeSearch(a.producteurName).includes(q) ||
        this.normalizeSearch(a.product).includes(q) ||
        this.normalizeSearch(a.zone).includes(q) ||
        (a.description && this.normalizeSearch(a.description).includes(q))
      );
    }
    if (region) {
      data = data.filter(a => a.zone.toLowerCase().includes(region.toLowerCase()));
    }
    if (ville) {
      data = data.filter(a => a.zone.toLowerCase().includes(ville.toLowerCase()));
    }
    return data;
  });

  readonly filteredPoussins = computed(() => {
    const region = this.selectedRegion();
    const ville = this.selectedVille();
    const race = this.selectedRace();
    const sort = this.sortQuantity();
    const query = this.debouncedSearchQuery();
    let data = this.poussins();

    if (query) {
      const q = this.normalizeSearch(query);
      data = data.filter(p =>
        this.normalizeSearch(p.producteur).includes(q) ||
        this.normalizeSearch(p.race).includes(q) ||
        this.normalizeSearch(p.location).includes(q) ||
        (p.vaccinationDetails && this.normalizeSearch(p.vaccinationDetails).includes(q))
      );
    }
    if (region) {
      data = data.filter(p => p.region.toLowerCase().includes(region.toLowerCase())
        || this.regionContainsLocation(region, p.location));
    }
    if (ville) {
      data = data.filter(p => p.location.toLowerCase().includes(ville.toLowerCase()));
    }
    if (race && race !== 'Toutes') {
      data = data.filter(p => p.race.toLowerCase() === race.toLowerCase());
    }
    if (sort === 'asc') {
      data = [...data].sort((a, b) => a.quantity - b.quantity);
    } else if (sort === 'desc') {
      data = [...data].sort((a, b) => b.quantity - a.quantity);
    }
    return data;
  });

  /* Displayed (lazy-loaded) data */
  readonly displayedAnnonces = computed(() => {
    return this.filteredAnnonces().slice(0, this.annoncesDisplayCount());
  });

  readonly displayedBesoins = computed(() => {
    return this.filteredBesoins().slice(0, this.besoinsDisplayCount());
  });

  readonly displayedAliments = computed(() => {
    return this.filteredAliments().slice(0, this.alimentsDisplayCount());
  });

  readonly displayedPoussins = computed(() => {
    return this.filteredPoussins().slice(0, this.poussinsDisplayCount());
  });

  /* Ticker: duplicate deliveries for seamless scroll */
  readonly tickerDeliveries = computed(() => {
    const d = this.upcomingDeliveries();
    return [...d, ...d];
  });

  /* ---- Auto-scroll state ---- */
  readonly offresScrollOffset = signal(0);
  readonly demandesScrollOffset = signal(0);
  readonly alimentsScrollOffset = signal(0);
  readonly offresTransition = signal('transform 0.8s cubic-bezier(0.4, 0, 0.2, 1)');
  readonly demandesTransition = signal('transform 0.8s cubic-bezier(0.4, 0, 0.2, 1)');
  readonly alimentsTransition = signal('transform 0.8s cubic-bezier(0.4, 0, 0.2, 1)');
  readonly poussinsScrollOffset = signal(0);
  readonly poussinsTransition = signal('transform 0.8s cubic-bezier(0.4, 0, 0.2, 1)');

  private offresScrollIndex = 0;
  private demandesScrollIndex = 0;
  private alimentsScrollIndex = 0;
  private poussinsScrollIndex = 0;
  private scrollIntervalId: ReturnType<typeof setInterval> | null = null;

  /* ---- Refresh timer ---- */
  readonly secondsSinceUpdate = signal(0);
  private refreshCounterId: ReturnType<typeof setInterval> | null = null;

  /* ---- Detail panel ---- */
  readonly detailOpen = signal(false);
  readonly detailType = signal<'annonce' | 'besoin' | 'aliment' | 'poussin'>('annonce');
  readonly detailTitle = signal('');
  readonly detailAnnonce = signal<Annonce | null>(null);
  readonly detailBesoin = signal<Besoin | null>(null);
  readonly detailAliment = signal<Aliment | null>(null);
  readonly detailPoussin = signal<Poussin | null>(null);

  private readonly ROW_HEIGHT = 48;
  private readonly VISIBLE_ROWS_MAIN = 10;
  private readonly VISIBLE_ROWS_SHORT = 5;
  private readonly API_BASE = 'http://localhost:8901/api/public/dashboard';
  private previousAnnonceCount = 0;
  private previousBesoinCount = 0;
  private previousAlimentCount = 0;
  private previousPoussinCount = 0;

  /* ================================================================
     LIFECYCLE
     ================================================================ */
  ngOnInit(): void {
    if (!isPlatformBrowser(this.platformId)) return;

    // Load sound preference from localStorage
    const savedSound = localStorage.getItem('poulets_sound_enabled');
    if (savedSound === 'true') {
      this.soundEnabled.set(true);
    }

    // Initialize notification audio
    this.notificationAudio = new Audio(NOTIFICATION_SOUND_BASE64);
    this.notificationAudio.volume = 0.3;

    // Store initial counts
    this.previousAnnonceCount = this.annonces().length;
    this.previousBesoinCount = this.besoins().length;
    this.previousAlimentCount = this.aliments().length;
    this.previousPoussinCount = this.poussins().length;

    // Start data fetching with auto-refresh every 15s
    this.startDataPolling();

    // Refresh counter (seconds since last update)
    this.refreshCounterId = setInterval(() => {
      this.zone.run(() => {
        this.secondsSinceUpdate.update(v => v + 1);
      });
    }, 1000);

    // Show validated contracts with auto-dismiss
    this.showValidatedContracts();

    // Animate stats on init
    setTimeout(() => this.animateStats(), 500);
  }

  ngAfterViewInit(): void {
    if (!isPlatformBrowser(this.platformId)) return;

    // Start auto-scroll every 5 seconds
    this.zone.runOutsideAngular(() => {
      this.scrollIntervalId = setInterval(() => {
        this.zone.run(() => {
          this.scrollBoard('offres');
          this.scrollBoard('demandes');
          this.scrollBoard('aliments');
          this.scrollBoard('poussins');
        });
      }, 5000);
    });
  }

  ngOnDestroy(): void {
    this.destroy$.next();
    this.destroy$.complete();
    if (this.scrollIntervalId) clearInterval(this.scrollIntervalId);
    if (this.refreshCounterId) clearInterval(this.refreshCounterId);
    if (this.searchDebounceTimer) clearTimeout(this.searchDebounceTimer);
    this.contractDismissTimers.forEach(t => clearTimeout(t));
  }

  /* ================================================================
     DATA FETCHING
     ================================================================ */
  private startDataPolling(): void {
    // Annonces
    interval(15000).pipe(
      startWith(0),
      switchMap(() =>
        this.http.get<Annonce[]>(`${this.API_BASE}/annonces?limit=40`).pipe(
          catchError(() => of(null))
        )
      ),
      tap(() => this.secondsSinceUpdate.set(0)),
      takeUntil(this.destroy$),
    ).subscribe(data => {
      if (data) {
        const newCount = data.length;
        if (newCount > this.previousAnnonceCount && this.previousAnnonceCount > 0) {
          this.playNotificationSound();
        }
        this.previousAnnonceCount = newCount;
        this.annonces.set(data);
      }
    });

    // Besoins
    interval(15000).pipe(
      startWith(0),
      switchMap(() =>
        this.http.get<Besoin[]>(`${this.API_BASE}/besoins?limit=40`).pipe(
          catchError(() => of(null))
        )
      ),
      takeUntil(this.destroy$),
    ).subscribe(data => {
      if (data) {
        const newCount = data.length;
        if (newCount > this.previousBesoinCount && this.previousBesoinCount > 0) {
          this.playNotificationSound();
        }
        this.previousBesoinCount = newCount;
        this.besoins.set(data);
      }
    });

    // Aliments
    interval(15000).pipe(
      startWith(0),
      switchMap(() =>
        this.http.get<Aliment[]>(`${this.API_BASE}/aliments?limit=20`).pipe(
          catchError(() => of(null))
        )
      ),
      takeUntil(this.destroy$),
    ).subscribe(data => {
      if (data) {
        const newCount = data.length;
        if (newCount > this.previousAlimentCount && this.previousAlimentCount > 0) {
          this.playNotificationSound();
        }
        this.previousAlimentCount = newCount;
        this.aliments.set(data);
      }
    });

    // Poussins
    interval(15000).pipe(
      startWith(0),
      switchMap(() =>
        this.http.get<Poussin[]>(`${this.API_BASE}/poussins?limit=20`).pipe(
          catchError(() => of(null))
        )
      ),
      takeUntil(this.destroy$),
    ).subscribe(data => {
      if (data) {
        const newCount = data.length;
        if (newCount > this.previousPoussinCount && this.previousPoussinCount > 0) {
          this.playNotificationSound();
        }
        this.previousPoussinCount = newCount;
        this.poussins.set(data);
      }
    });

    // Stats
    interval(15000).pipe(
      startWith(0),
      switchMap(() =>
        this.http.get<DashboardStats>(`${this.API_BASE}/stats`).pipe(
          catchError(() => of(null))
        )
      ),
      takeUntil(this.destroy$),
    ).subscribe(data => {
      if (data) {
        this.stats.set(data);
        this.animateStats();
      }
    });
  }

  /* ================================================================
     SOUND
     ================================================================ */
  toggleSound(enabled: boolean): void {
    this.soundEnabled.set(enabled);
    localStorage.setItem('poulets_sound_enabled', String(enabled));
  }

  private playNotificationSound(): void {
    if (this.soundEnabled() && this.notificationAudio) {
      this.notificationAudio.currentTime = 0;
      this.notificationAudio.play().catch(() => { /* ignore autoplay errors */ });
    }
  }

  /* ================================================================
     LAZY LOADING
     ================================================================ */
  loadMoreAnnonces(): void {
    this.annoncesDisplayCount.update(c => c + 20);
  }

  loadMoreBesoins(): void {
    this.besoinsDisplayCount.update(c => c + 20);
  }

  loadMoreAliments(): void {
    this.alimentsDisplayCount.update(c => c + 20);
  }

  loadMorePoussins(): void {
    this.poussinsDisplayCount.update(c => c + 20);
  }

  /* ================================================================
     SEARCH
     ================================================================ */
  onSearch(event: Event): void {
    const input = event.target as HTMLInputElement;
    const value = input.value;
    this.searchQuery.set(value);
    if (this.searchDebounceTimer) {
      clearTimeout(this.searchDebounceTimer);
    }
    this.searchDebounceTimer = setTimeout(() => {
      this.debouncedSearchQuery.set(value);
      this.resetScrollAndCounts();
    }, 300);
  }

  clearSearch(): void {
    this.searchQuery.set('');
    this.debouncedSearchQuery.set('');
    if (this.searchDebounceTimer) {
      clearTimeout(this.searchDebounceTimer);
    }
    this.resetScrollAndCounts();
  }

  /* ================================================================
     FILTERS
     ================================================================ */
  onRegionChange(event: Event): void {
    const select = event.target as HTMLSelectElement;
    this.selectedRegion.set(select.value);
    this.resetScrollAndCounts();
  }

  onVilleChange(event: Event): void {
    const select = event.target as HTMLSelectElement;
    this.selectedVille.set(select.value);
    this.resetScrollAndCounts();
  }

  onRaceChange(event: Event): void {
    const select = event.target as HTMLSelectElement;
    this.selectedRace.set(select.value);
    this.resetScrollAndCounts();
  }

  onSortQuantityChange(event: Event): void {
    const select = event.target as HTMLSelectElement;
    this.sortQuantity.set(select.value as '' | 'asc' | 'desc');
  }

  setRoleTab(tab: 'all' | 'eleveurs' | 'clients' | 'producteurs' | 'poussins'): void {
    this.activeRoleTab.set(tab);
    this.resetScrollAndCounts();
  }

  private resetScrollAndCounts(): void {
    this.setScrollOffset('offres', 0, 0);
    this.setScrollOffset('demandes', 0, 0);
    this.setScrollOffset('aliments', 0, 0);
    this.setScrollOffset('poussins', 0, 0);
    this.annoncesDisplayCount.set(20);
    this.besoinsDisplayCount.set(20);
    this.alimentsDisplayCount.set(20);
    this.poussinsDisplayCount.set(20);
  }

  /* ================================================================
     STATS ANIMATION
     ================================================================ */
  private animateStats(): void {
    const stats = this.stats();
    this.animateCountUp(this.animatedMatchings, stats.activeMatchings, 1200);
    this.animateCountUp(this.animatedLiveUsers, stats.liveUsers, 1500);
    this.animateCountUp(this.animatedContractsToday, stats.contractsToday, 1000);
    this.animateCountUp(this.animatedNewBesoins, stats.newBesoins, 1100);
  }

  private animateCountUp(sig: ReturnType<typeof signal<number>>, target: number, duration: number): void {
    const start = sig();
    if (start === target) return;
    const startTime = performance.now();

    const step = (now: number) => {
      const elapsed = now - startTime;
      const progress = Math.min(elapsed / duration, 1);
      // Ease out cubic
      const eased = 1 - Math.pow(1 - progress, 3);
      const current = Math.round(start + (target - start) * eased);
      this.zone.run(() => sig.set(current));
      if (progress < 1) {
        requestAnimationFrame(step);
      }
    };
    requestAnimationFrame(step);
  }

  /* ================================================================
     CONTRACTS
     ================================================================ */
  private showValidatedContracts(): void {
    const contracts = this.validatedContracts();
    this.visibleContracts.set([...contracts]);
    // Auto-dismiss each after 8 seconds
    contracts.forEach((contract, i) => {
      const timer = setTimeout(() => {
        this.zone.run(() => {
          this.visibleContracts.update(list => list.filter(c => c.id !== contract.id));
        });
      }, 8000 + i * 2000);
      this.contractDismissTimers.push(timer);
    });
  }

  /* ================================================================
     AUTO-SCROLL
     ================================================================ */
  private scrollBoard(board: 'offres' | 'demandes' | 'aliments' | 'poussins'): void {
    const maxVisible = (board === 'aliments' || board === 'poussins') ? this.VISIBLE_ROWS_SHORT : this.VISIBLE_ROWS_MAIN;
    let totalRows: number;
    let currentIndex: number;

    switch (board) {
      case 'offres':
        totalRows = this.displayedAnnonces().length;
        currentIndex = this.offresScrollIndex;
        break;
      case 'demandes':
        totalRows = this.displayedBesoins().length;
        currentIndex = this.demandesScrollIndex;
        break;
      case 'aliments':
        totalRows = this.displayedAliments().length;
        currentIndex = this.alimentsScrollIndex;
        break;
      case 'poussins':
        totalRows = this.displayedPoussins().length;
        currentIndex = this.poussinsScrollIndex;
        break;
    }

    if (totalRows <= maxVisible) {
      this.setScrollOffset(board, 0, 0);
      return;
    }

    const maxScroll = totalRows - maxVisible;
    let nextIndex = currentIndex + 1;

    if (nextIndex > maxScroll) {
      this.setTransition(board, 'none');
      this.setScrollOffset(board, 0, 0);
      setTimeout(() => {
        this.zone.run(() => {
          this.setTransition(board, 'transform 0.8s cubic-bezier(0.4, 0, 0.2, 1)');
        });
      }, 50);
      return;
    }

    this.setTransition(board, 'transform 0.8s cubic-bezier(0.4, 0, 0.2, 1)');
    this.setScrollOffset(board, nextIndex, nextIndex * this.ROW_HEIGHT);
  }

  private setScrollOffset(board: string, index: number, px: number): void {
    switch (board) {
      case 'offres':
        this.offresScrollIndex = index;
        this.offresScrollOffset.set(px);
        break;
      case 'demandes':
        this.demandesScrollIndex = index;
        this.demandesScrollOffset.set(px);
        break;
      case 'aliments':
        this.alimentsScrollIndex = index;
        this.alimentsScrollOffset.set(px);
        break;
      case 'poussins':
        this.poussinsScrollIndex = index;
        this.poussinsScrollOffset.set(px);
        break;
    }
  }

  private setTransition(board: string, value: string): void {
    switch (board) {
      case 'offres': this.offresTransition.set(value); break;
      case 'demandes': this.demandesTransition.set(value); break;
      case 'aliments': this.alimentsTransition.set(value); break;
      case 'poussins': this.poussinsTransition.set(value); break;
    }
  }

  /* ================================================================
     ACTIONS
     ================================================================ */
  openDetails(type: 'annonce' | 'besoin' | 'aliment' | 'poussin', item: any): void {
    if (!this.authService.isLoggedIn()) {
      this.redirectToLogin(type, item);
      return;
    }
    this.detailType.set(type);
    this.detailAnnonce.set(null);
    this.detailBesoin.set(null);
    this.detailAliment.set(null);
    this.detailPoussin.set(null);

    switch (type) {
      case 'annonce':
        this.detailAnnonce.set(item);
        this.detailTitle.set(`Offre - ${item.race}`);
        break;
      case 'besoin':
        this.detailBesoin.set(item);
        this.detailTitle.set(`Demande - ${item.race}`);
        break;
      case 'aliment':
        this.detailAliment.set(item);
        this.detailTitle.set(`Aliment - ${item.product}`);
        break;
      case 'poussin':
        this.detailPoussin.set(item);
        this.detailTitle.set(`Poussin - ${item.race}`);
        break;
    }
    this.detailOpen.set(true);
  }

  closeDetails(): void {
    this.detailOpen.set(false);
  }

  contactUser(type: string, item: any): void {
    this.redirectToLogin(type, item);
  }

  contactFromDetail(): void {
    const type = this.detailType();
    let item: any;
    switch (type) {
      case 'annonce': item = this.detailAnnonce(); break;
      case 'besoin': item = this.detailBesoin(); break;
      case 'aliment': item = this.detailAliment(); break;
      case 'poussin': item = this.detailPoussin(); break;
    }
    if (item) {
      this.closeDetails();
      if (!this.authService.isLoggedIn()) {
        this.redirectToLogin(type, item);
      } else {
        this.contactUser(type, item);
      }
    }
  }

  private redirectToLogin(type: string, item: any): void {
    const returnUrl = type === 'annonce'
      ? `/marketplace/annonces/${item.id}`
      : type === 'besoin'
        ? `/marketplace/besoins/${item.id}`
        : type === 'poussin'
          ? `/marketplace/poussins/${item.id}`
          : `/marketplace/aliments/${item.id}`;

    this.router.navigate(['/auth/login'], {
      queryParams: { returnUrl }
    });
  }

  /* ================================================================
     HELPERS
     ================================================================ */
  isNew(createdAt: string): boolean {
    const created = new Date(createdAt).getTime();
    const oneHourAgo = Date.now() - 60 * 60 * 1000;
    return created > oneHourAgo;
  }

  isVeryNew(createdAt: string): boolean {
    const created = new Date(createdAt).getTime();
    const twoHoursAgo = Date.now() - 2 * 60 * 60 * 1000;
    return created > twoHoursAgo && this.isNew(createdAt);
  }

  formatNumber(n: number): string {
    return n.toLocaleString('fr-FR');
  }

  getStatusLabel(status: string): string {
    switch (status) {
      case 'available': return 'Dispo';
      case 'partial': return 'Partiel';
      case 'sold': return 'Vendu';
      default: return status;
    }
  }

  getTypeIcon(type: string): string {
    switch (type) {
      case 'restaurant': return 'restaurant';
      case 'menage': return 'home';
      case 'evenement': return 'celebration';
      case 'revendeur': return 'store';
      default: return 'person';
    }
  }

  getTypeLabel(type: string): string {
    switch (type) {
      case 'restaurant': return 'Restaurant';
      case 'menage': return 'M\u00e9nage';
      case 'evenement': return '\u00c9v\u00e9nement';
      case 'revendeur': return 'Revendeur';
      default: return type;
    }
  }

  getFreqLabel(freq: string): string {
    switch (freq) {
      case 'hebdo': return 'Chq Ven';
      case 'mensuel': return 'Mensuel';
      case 'ponctuel': return 'Ponctuel';
      default: return freq;
    }
  }

  getStockPercent(stock: number): number {
    return Math.min(100, (stock / 500) * 100);
  }

  /**
   * Normalize text for accent-insensitive, case-insensitive search.
   */
  private normalizeSearch(text: string): string {
    return text
      .toLowerCase()
      .normalize('NFD')
      .replace(/[\u0300-\u036f]/g, '');
  }

  private regionContainsLocation(region: string, location: string): boolean {
    const regionCities: Record<string, string[]> = {
      'Centre': ['Ouagadougou', 'Ziniar\u00e9'],
      'Hauts-Bassins': ['Bobo-Dioulasso'],
      'Centre-Ouest': ['Koudougou'],
      'Nord': ['Ouahigouya'],
      'Cascades': ['Banfora'],
      'Centre-Est': ['Tenkodogo'],
      'Centre-Nord': ['Kaya'],
      'Boucle du Mouhoun': ['D\u00e9dougou', 'Dedougou'],
      'Est': ['Fada', 'Fada N\'Gourma'],
      'Plateau-Central': ['Ziniar\u00e9'],
      'Sud-Ouest': ['L\u00e9o', 'Leo'],
      'Sahel': ['Dori'],
      'Centre-Sud': ['Manga'],
    };
    const cities = regionCities[region] || [];
    return cities.some(city => location.toLowerCase().includes(city.toLowerCase()));
  }
}
