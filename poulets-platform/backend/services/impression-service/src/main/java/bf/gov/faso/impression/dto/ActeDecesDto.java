package bf.gov.faso.impression.dto;

import java.time.LocalDate;
import java.util.List;

/**
 * DTO representant les donnees d'un Extrait d'Acte de Deces
 * pour le template FreeMarker (Flying Saucer ITextRenderer).
 *
 * Certificat de Deces — Burkina Faso.
 */
public record ActeDecesDto(

    // ===== NUMERO ET DATE DE L'ACTE =====
    String numero,
    LocalDate dateActe,

    // ===== LOCALISATION ADMINISTRATIVE =====
    String region,
    String province,
    String commune,

    // ===== IDENTITE DU DEFUNT =====
    String nomDefunt,
    String prenomsDefunt,
    String sexeDefunt,
    LocalDate dateNaissanceDefunt,
    String dateNaissanceDefuntEnLettres,
    String lieuNaissanceDefunt,
    String professionDefunt,
    String domicileDefunt,
    String situationMatrimoniale,
    String nationaliteDefunt,

    // ===== PARENTS DU DEFUNT =====
    String nomPereDefunt,
    String nomMereDefunt,

    // ===== INFORMATIONS DECES =====
    LocalDate dateDeces,
    String dateDecesEnLettres,
    String heureDeces,
    String lieuDeces,
    String causeDeces,

    // ===== INFORMATIONS MEDICALES =====
    String medecinCertificateur,
    String numeroCertificatMedical,
    LocalDate dateCertificatMedical,
    String etablissementSante,

    // ===== DECLARANT =====
    String nomDeclarant,
    String lienDeclarant,
    String adresseDeclarant,

    // ===== DELIVRANCE =====
    String lieuDelivrance,
    LocalDate dateDelivrance,
    String nomOfficier,

    // ===== RESSOURCES (chemins images) =====
    String signatureUrl,
    String qrCodeUrl,

    // ===== SECURITE ET VERIFICATION =====
    String codeVerification,
    String referenceUnique,
    String documentHash,

    // ===== MENTIONS MARGINALES =====
    List<String> mentionsMarginales
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

        // Identite du defunt
        private String nomDefunt;
        private String prenomsDefunt;
        private String sexeDefunt;
        private LocalDate dateNaissanceDefunt;
        private String dateNaissanceDefuntEnLettres;
        private String lieuNaissanceDefunt;
        private String professionDefunt;
        private String domicileDefunt;
        private String situationMatrimoniale;
        private String nationaliteDefunt = "Burkinab\u00e8";

        // Parents du defunt
        private String nomPereDefunt;
        private String nomMereDefunt;

        // Informations deces
        private LocalDate dateDeces;
        private String dateDecesEnLettres;
        private String heureDeces;
        private String lieuDeces;
        private String causeDeces;

        // Informations medicales
        private String medecinCertificateur;
        private String numeroCertificatMedical;
        private LocalDate dateCertificatMedical;
        private String etablissementSante;

        // Declarant
        private String nomDeclarant;
        private String lienDeclarant;
        private String adresseDeclarant;

        // Delivrance
        private String lieuDelivrance = "Ouagadougou";
        private LocalDate dateDelivrance;
        private String nomOfficier;

        // Ressources
        private String signatureUrl;
        private String qrCodeUrl;

        // Securite et verification
        private String codeVerification;
        private String referenceUnique;
        private String documentHash;

        // Mentions marginales
        private List<String> mentionsMarginales;

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

        // ===== IDENTITE DU DEFUNT =====

        public Builder nomDefunt(String nomDefunt) {
            this.nomDefunt = nomDefunt;
            return this;
        }

        public Builder prenomsDefunt(String prenomsDefunt) {
            this.prenomsDefunt = prenomsDefunt;
            return this;
        }

        public Builder sexeDefunt(String sexeDefunt) {
            this.sexeDefunt = sexeDefunt;
            return this;
        }

        public Builder dateNaissanceDefunt(LocalDate dateNaissanceDefunt) {
            this.dateNaissanceDefunt = dateNaissanceDefunt;
            return this;
        }

        public Builder dateNaissanceDefuntEnLettres(String dateNaissanceDefuntEnLettres) {
            this.dateNaissanceDefuntEnLettres = dateNaissanceDefuntEnLettres;
            return this;
        }

        public Builder lieuNaissanceDefunt(String lieuNaissanceDefunt) {
            this.lieuNaissanceDefunt = lieuNaissanceDefunt;
            return this;
        }

        public Builder professionDefunt(String professionDefunt) {
            this.professionDefunt = professionDefunt;
            return this;
        }

        public Builder domicileDefunt(String domicileDefunt) {
            this.domicileDefunt = domicileDefunt;
            return this;
        }

        public Builder situationMatrimoniale(String situationMatrimoniale) {
            this.situationMatrimoniale = situationMatrimoniale;
            return this;
        }

        public Builder nationaliteDefunt(String nationaliteDefunt) {
            this.nationaliteDefunt = nationaliteDefunt;
            return this;
        }

        // ===== PARENTS DU DEFUNT =====

        public Builder nomPereDefunt(String nomPereDefunt) {
            this.nomPereDefunt = nomPereDefunt;
            return this;
        }

        public Builder nomMereDefunt(String nomMereDefunt) {
            this.nomMereDefunt = nomMereDefunt;
            return this;
        }

        // ===== INFORMATIONS DECES =====

        public Builder dateDeces(LocalDate dateDeces) {
            this.dateDeces = dateDeces;
            return this;
        }

        public Builder dateDecesEnLettres(String dateDecesEnLettres) {
            this.dateDecesEnLettres = dateDecesEnLettres;
            return this;
        }

        public Builder heureDeces(String heureDeces) {
            this.heureDeces = heureDeces;
            return this;
        }

        public Builder lieuDeces(String lieuDeces) {
            this.lieuDeces = lieuDeces;
            return this;
        }

        public Builder causeDeces(String causeDeces) {
            this.causeDeces = causeDeces;
            return this;
        }

        // ===== INFORMATIONS MEDICALES =====

        public Builder medecinCertificateur(String medecinCertificateur) {
            this.medecinCertificateur = medecinCertificateur;
            return this;
        }

        public Builder numeroCertificatMedical(String numeroCertificatMedical) {
            this.numeroCertificatMedical = numeroCertificatMedical;
            return this;
        }

        public Builder dateCertificatMedical(LocalDate dateCertificatMedical) {
            this.dateCertificatMedical = dateCertificatMedical;
            return this;
        }

        public Builder etablissementSante(String etablissementSante) {
            this.etablissementSante = etablissementSante;
            return this;
        }

        // ===== DECLARANT =====

        public Builder nomDeclarant(String nomDeclarant) {
            this.nomDeclarant = nomDeclarant;
            return this;
        }

        public Builder lienDeclarant(String lienDeclarant) {
            this.lienDeclarant = lienDeclarant;
            return this;
        }

        public Builder adresseDeclarant(String adresseDeclarant) {
            this.adresseDeclarant = adresseDeclarant;
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

        // ===== RESSOURCES =====

        public Builder signatureUrl(String signatureUrl) {
            this.signatureUrl = signatureUrl;
            return this;
        }

        public Builder qrCodeUrl(String qrCodeUrl) {
            this.qrCodeUrl = qrCodeUrl;
            return this;
        }

        // ===== SECURITE ET VERIFICATION =====

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

        // ===== MENTIONS MARGINALES =====

        public Builder mentionsMarginales(List<String> mentionsMarginales) {
            this.mentionsMarginales = mentionsMarginales;
            return this;
        }

        /**
         * Construit l'instance immutable du DTO.
         */
        public ActeDecesDto build() {
            return new ActeDecesDto(
                this.numero,
                this.dateActe,
                this.region,
                this.province,
                this.commune,
                this.nomDefunt,
                this.prenomsDefunt,
                this.sexeDefunt,
                this.dateNaissanceDefunt,
                this.dateNaissanceDefuntEnLettres,
                this.lieuNaissanceDefunt,
                this.professionDefunt,
                this.domicileDefunt,
                this.situationMatrimoniale,
                this.nationaliteDefunt,
                this.nomPereDefunt,
                this.nomMereDefunt,
                this.dateDeces,
                this.dateDecesEnLettres,
                this.heureDeces,
                this.lieuDeces,
                this.causeDeces,
                this.medecinCertificateur,
                this.numeroCertificatMedical,
                this.dateCertificatMedical,
                this.etablissementSante,
                this.nomDeclarant,
                this.lienDeclarant,
                this.adresseDeclarant,
                this.lieuDelivrance,
                this.dateDelivrance,
                this.nomOfficier,
                this.signatureUrl,
                this.qrCodeUrl,
                this.codeVerification,
                this.referenceUnique,
                this.documentHash,
                this.mentionsMarginales
            );
        }
    }
}
