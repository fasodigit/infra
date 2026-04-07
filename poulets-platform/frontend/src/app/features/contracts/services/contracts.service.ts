import { Injectable, inject } from '@angular/core';
import { Apollo } from 'apollo-angular';
import { Observable, map, filter as rxFilter } from 'rxjs';
import {
  Contract,
  ContractPerformance,
  ContractFilter,
  CreateContractInput,
  ContractDuration,
} from '../../../shared/models/contract.models';
import { Page } from '../../../services/graphql.service';
import {
  GET_CONTRACTS,
  GET_CONTRACT_BY_ID,
  GET_CONTRACT_PERFORMANCE,
  CREATE_CONTRACT,
  SIGN_CONTRACT,
  RENEW_CONTRACT,
  TERMINATE_CONTRACT,
  SEARCH_PARTNERS,
} from '../graphql/contracts.graphql';

export interface PartnerSearchResult {
  id: string;
  nom: string;
  prenom?: string;
  role: string;
  localisation: string;
  note: number;
}

@Injectable({ providedIn: 'root' })
export class ContractsService {
  private readonly apollo = inject(Apollo);

  getContracts(filter?: ContractFilter, page = 0, size = 20): Observable<Page<Contract>> {
    return this.apollo
      .watchQuery<{ contracts: Page<Contract> }>({
        query: GET_CONTRACTS,
        variables: { filter, page, size },
      })
      .valueChanges.pipe(
        rxFilter((r) => !!r.data),
        map((r) => r.data!.contracts as Page<Contract>),
      );
  }

  getContractById(id: string): Observable<Contract> {
    return this.apollo
      .watchQuery<{ contract: Contract }>({
        query: GET_CONTRACT_BY_ID,
        variables: { id },
      })
      .valueChanges.pipe(
        rxFilter((r) => !!r.data),
        map((r) => r.data!.contract as Contract),
      );
  }

  getContractPerformance(contractId: string): Observable<ContractPerformance> {
    return this.apollo
      .watchQuery<{ contractPerformance: ContractPerformance }>({
        query: GET_CONTRACT_PERFORMANCE,
        variables: { contractId },
      })
      .valueChanges.pipe(
        rxFilter((r) => !!r.data),
        map((r) => r.data!.contractPerformance as ContractPerformance),
      );
  }

  createContract(input: CreateContractInput): Observable<Contract> {
    return this.apollo
      .mutate<{ createContract: Contract }>({
        mutation: CREATE_CONTRACT,
        variables: { input },
        refetchQueries: [{ query: GET_CONTRACTS }],
      })
      .pipe(map((r) => r.data!.createContract));
  }

  signContract(contractId: string): Observable<Contract> {
    return this.apollo
      .mutate<{ signContract: Contract }>({
        mutation: SIGN_CONTRACT,
        variables: { contractId },
      })
      .pipe(map((r) => r.data!.signContract));
  }

  renewContract(contractId: string, newDuration: ContractDuration): Observable<Contract> {
    return this.apollo
      .mutate<{ renewContract: Contract }>({
        mutation: RENEW_CONTRACT,
        variables: { contractId, newDuration },
      })
      .pipe(map((r) => r.data!.renewContract));
  }

  terminateContract(contractId: string, reason?: string): Observable<Contract> {
    return this.apollo
      .mutate<{ terminateContract: Contract }>({
        mutation: TERMINATE_CONTRACT,
        variables: { contractId, reason },
      })
      .pipe(map((r) => r.data!.terminateContract));
  }

  searchPartners(query: string, role?: string): Observable<PartnerSearchResult[]> {
    return this.apollo
      .watchQuery<{ searchPartners: PartnerSearchResult[] }>({
        query: SEARCH_PARTNERS,
        variables: { query, role },
        fetchPolicy: 'network-only',
      })
      .valueChanges.pipe(
        rxFilter((r) => !!r.data),
        map((r) => r.data!.searchPartners as PartnerSearchResult[]),
      );
  }
}
