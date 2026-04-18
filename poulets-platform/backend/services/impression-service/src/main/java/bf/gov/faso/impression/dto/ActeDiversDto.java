package bf.gov.faso.impression.dto;

import java.time.LocalDate;
import java.util.List;

/**
 * DTO representant les donnees d'un Acte Divers (actes administratifs divers)
 * pour le template FreeMarker (Flying Saucer ITextRenderer).
 *
 * Couvre : certificat de residence, certificat de vie, certificat de celibat,
 * certificat de non-divorce, certificat de nationalite burkinabe,
 * legalisation de signature, certificat de bonne vie et moeurs,
 * attestation de prise en charge, certificat de coutume, etc.
 */
public record ActeDiversDto(

    // ===== NUMERO ET DATE DE L'ACTE =====
    String numero,
    LocalDate dateActe,

    // ===== LOCALISATION ADMINISTRATIVE =====
    String region,
    String province,
    String commune,

    // ===== TYPE D'ACTE DIVERS =====
    String typeActeDivers,

    // ===== BENEFICIAIRE =====
    String nom,
    String prenoms,
    String sexe,
    LocalDate dateNaissance,
    String dateNaissanceEnLettres,
    String lieuNaissance,
    String nationalite,
    String profession,
    String domicile,
    String numeroCnib,

    // ===== CONTENU DE L'ACTE =====
    String objetActe,
    String contenuPrincipal,
    String motif,
    String observations,
    List<String> mentionsComplementaires,

    // ===== DEMANDEUR (si different du beneficiaire) =====
    String nomDemandeur,
    String lienDemandeur,

    // ===== DELIVRANCE =====
    String lieuDelivrance,
    LocalDate dateDelivrance,
    String nomOfficier,
    String signatureUrl,
    String qrCodeUrl,
    String codeVerification,
    String referenceUnique,
    String documentHash
) {

    /**
     * Builder statique pour construction flexible du DTO.
     */
    public static Builder builder() {
        return new Builder();
    }

    public static final class Builder {

        // Numero et date de l'acte
        private String numero;
        private LocalDate dateActe;

        // Localisation administrative
        private String region = "REGION DU CENTRE";
        private String province = "PROVINCE DU KADIOGO";
        private String commune = "OUAGADOUGOU";

        // Type d'acte divers
        private String typeActeDivers = "ACTE DIVERS";

        // Beneficiaire
        private String nom;
        private String prenoms;
        private String sexe;
        private LocalDate dateNaissance;
        private String dateNaissanceEnLettres;
        private String lieuNaissance;
        private String nationalite = "Burkinab\u00e8";
        private String profession;
        private String domicile;
        private String numeroCnib;

        // Contenu de l'acte
        private String objetActe;
        private String contenuPrincipal;
        private String motif;
        private String observations;
        private List<String> mentionsComplementaires;

        // Demandeur
        private String nomDemandeur;
        private String lienDemandeur;

        // Delivrance
        private String lieuDelivrance = "Ouagadougou";
        private LocalDate dateDelivrance;
        private String nomOfficier;
        private String signatureUrl;
        private String qrCodeUrl;
        private String codeVerification;
        private String referenceUnique;
        private String documentHash;

        private Builder() {}

        // ===== NUMERO ET DATE DE L'ACTE =====

        public Builder numero(String numero) {
            this.numero = numero;
            return this;
        }

        public Builder dateActe(LocalDate dateActe) {
            this.dateActe = dateActe;
            return this;
        }

        // ===== LOCALISATION ADMINISTRATIVE =====

        public Builder region(String region) {
            this.region = region;
            return this;
        }

        public Builder province(String province) {
            this.province = province;
            return this;
        }

        public Builder commune(String commune) {
            this.commune = commune;
            return this;
        }

        // ===== TYPE D'ACTE DIVERS =====

        public Builder typeActeDivers(String typeActeDivers) {
            this.typeActeDivers = typeActeDivers;
            return this;
        }

        // ===== BENEFICIAIRE =====

        public Builder nom(String nom) {
            this.nom = nom;
            return this;
        }

        public Builder prenoms(String prenoms) {
            this.prenoms = prenoms;
            return this;
        }

        public Builder sexe(String sexe) {
            this.sexe = sexe;
            return this;
        }

        public Builder dateNaissance(LocalDate dateNaissance) {
            this.dateNaissance = dateNaissance;
            return this;
        }

        public Builder dateNaissanceEnLettres(String dateNaissanceEnLettres) {
            this.dateNaissanceEnLettres = dateNaissanceEnLettres;
            return this;
        }

        public Builder lieuNaissance(String lieuNaissance) {
            this.lieuNaissance = lieuNaissance;
            return this;
        }

        public Builder nationalite(String nationalite) {
            this.nationalite = nationalite;
            return this;
        }

        public Builder profession(String profession) {
            this.profession = profession;
            return this;
        }

        public Builder domicile(String domicile) {
            this.domicile = domicile;
            return this;
        }

        public Builder numeroCnib(String numeroCnib) {
            this.numeroCnib = numeroCnib;
            return this;
        }

        // ===== CONTENU DE L'ACTE =====

        public Builder objetActe(String objetActe) {
            this.objetActe = objetActe;
            return this;
        }

        public Builder contenuPrincipal(String contenuPrincipal) {
            this.contenuPrincipal = contenuPrincipal;
            return this;
        }

        public Builder motif(String motif) {
            this.motif = motif;
            return this;
        }

        public Builder observations(String observations) {
            this.observations = observations;
            return this;
        }

        public Builder mentionsComplementaires(List<String> mentionsComplementaires) {
            this.mentionsComplementaires = mentionsComplementaires;
            return this;
        }

        // ===== DEMANDEUR =====

        public Builder nomDemandeur(String nomDemandeur) {
            this.nomDemandeur = nomDemandeur;
            return this;
        }

        public Builder lienDemandeur(String lienDemandeur) {
            this.lienDemandeur = lienDemandeur;
            return this;
        }

        // ===== DELIVRANCE =====

        public Builder lieuDelivrance(String lieuDelivrance) {
            this.lieuDelivrance = lieuDelivrance;
            return this;
        }

        public Builder dateDelivrance(LocalDate dateDelivrance) {
            this.dateDelivrance = dateDelivrance;
            return this;
        }

        public Builder nomOfficier(String nomOfficier) {
            this.nomOfficier = nomOfficier;
            return this;
        }

        public Builder signatureUrl(String signatureUrl) {
            this.signatureUrl = signatureUrl;
            return this;
        }

        public Builder qrCodeUrl(String qrCodeUrl) {
            this.qrCodeUrl = qrCodeUrl;
            return this;
        }

        public Builder codeVerification(String codeVerification) {
            this.codeVerification = codeVerification;
            return this;
        }

        public Builder referenceUnique(String referenceUnique) {
            this.referenceUnique = referenceUnique;
            return this;
        }

        public Builder documentHash(String documentHash) {
            this.documentHash = documentHash;
            return this;
        }

        /**
         * Construit l'instance immutable du DTO.
         */
        public ActeDiversDto build() {
            return new ActeDiversDto(
                this.numero,
                this.dateActe,
                this.region,
                this.province,
                this.commune,
                this.typeActeDivers,
                this.nom,
                this.prenoms,
                this.sexe,
                this.dateNaissance,
                this.dateNaissanceEnLettres,
                this.lieuNaissance,
                this.nationalite,
                this.profession,
                this.domicile,
                this.numeroCnib,
                this.objetActe,
                this.contenuPrincipal,
                this.motif,
                this.observations,
                this.mentionsComplementaires,
                this.nomDemandeur,
                this.lienDemandeur,
                this.lieuDelivrance,
                this.dateDelivrance,
                this.nomOfficier,
                this.signatureUrl,
                this.qrCodeUrl,
                this.codeVerification,
                this.referenceUnique,
                this.documentHash
            );
        }
    }
}
